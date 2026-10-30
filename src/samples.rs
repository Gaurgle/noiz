use std::f32::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The baked equal-power crossfade is this fraction of the trimmed sample,
/// clamped to `XFADE_MIN_SECS..=XFADE_MAX_SECS`, so the loop keeps most of the file.
const XFADE_FRACTION: f32 = 0.10;
const XFADE_MIN_SECS: f32 = 1.0;
const XFADE_MAX_SECS: f32 = 5.0;
/// Baked loops louder than this RMS level are attenuated to it; quieter ones are left alone.
const LOOP_RMS_DBFS: f32 = -30.0;
/// A window counts as body (not baked fade) when its RMS is at least this times the median.
const TRIM_RATIO: f32 = 0.7;

pub struct Sample {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Sample {
    pub fn from_wav(path: &Path) -> Option<Self> {
        let reader = hound::WavReader::open(path).ok()?;
        let spec = reader.spec();
        let channels = spec.channels;
        let sample_rate = spec.sample_rate;

        let data: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>().filter_map(|s| s.ok()).collect()
            }
            hound::SampleFormat::Int => {
                let max = (1 << (spec.bits_per_sample - 1)) as f32;
                reader.into_samples::<i32>().filter_map(|s| s.ok()).map(|s| s as f32 / max).collect()
            }
        };

        Some(Self { data, sample_rate, channels })
    }
}

/// A seamless loop baked at load time: interleaved stereo at the device rate.
/// Playing `frames` front to back and wrapping at `loop_len` is click-free.
pub struct LoopSample {
    pub frames: Box<[f32]>,
    pub loop_len: usize,
    /// Crossfade length in device frames.
    pub xfade: usize,
    /// Source frames removed by `trim_tail`.
    pub trimmed_frames: usize,
    /// Linear gain applied by level normalisation.
    pub gain: f32,
}

fn rms_envelope(data: &[f32], ch: usize, win: usize) -> Vec<f32> {
    data.chunks(win * ch)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect()
}

/// Cut a baked fade-out: keep everything before the last 50 ms window whose RMS
/// is at least `ratio` times the median window, minus one window of safety.
/// Returns the number of frames removed.
pub fn trim_tail(data: &mut Vec<f32>, ch: usize, rate: u32, ratio: f32) -> usize {
    let frames = data.len() / ch;
    let win = (rate as usize / 20).max(1);
    let env = rms_envelope(data, ch, win);
    if env.len() < 3 {
        return 0;
    }
    let mut sorted = env.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = sorted[sorted.len() / 2];
    let last_good = env.iter().rposition(|&e| e >= median * ratio).unwrap_or(env.len() - 1);
    let keep = (last_good.saturating_sub(1).max(1) * win).min(frames);
    data.truncate(keep * ch);
    frames - keep
}

/// Linear-interpolating resampler; adequate for broadband rain.
pub fn resample_linear(data: &[f32], ch: usize, from: u32, to: u32) -> Vec<f32> {
    // A zero rate (corrupt header) would overflow the output length; play it unresampled.
    if from == to || from == 0 || to == 0 || data.is_empty() {
        return data.to_vec();
    }
    let in_frames = data.len() / ch;
    let out_frames = (in_frames as f64 * to as f64 / from as f64) as usize;
    let step = from as f64 / to as f64;
    let mut out = Vec::with_capacity(out_frames * ch);
    for i in 0..out_frames {
        let pos = i as f64 * step;
        let i0 = (pos as usize).min(in_frames - 1);
        let i1 = (i0 + 1).min(in_frames - 1);
        let frac = (pos - i0 as f64) as f32;
        for c in 0..ch {
            let a = data[i0 * ch + c];
            let b = data[i1 * ch + c];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

/// Crossfade length in frames: `XFADE_FRACTION` of the sample clamped to
/// 1 to 5 s, and never more than a third of a very short sample.
fn xfade_frames(total: usize, rate: u32) -> usize {
    let min = (rate as f32 * XFADE_MIN_SECS) as usize;
    let max = (rate as f32 * XFADE_MAX_SECS) as usize;
    ((total as f32 * XFADE_FRACTION) as usize).clamp(min, max).min(total / 3).max(1)
}

/// Attenuation-only gain: brings a loop louder than `LOOP_RMS_DBFS` down to it.
/// Never boosts, so it cannot clip.
fn normalise_gain(frames: &[f32]) -> f32 {
    let rms = (frames.iter().map(|s| s * s).sum::<f32>() / frames.len().max(1) as f32).sqrt();
    if rms <= 0.0 {
        return 1.0;
    }
    (10f32.powf(LOOP_RMS_DBFS / 20.0) / rms).min(1.0)
}

/// Trim the baked fade, resample to `device_rate`, upmix to stereo, bake an
/// equal-power (sin/cos) crossfade across the loop point (see `xfade_frames`)
/// and normalise the level (see `normalise_gain`).
pub fn bake_loop(sample: &Sample, device_rate: u32) -> LoopSample {
    let ch = sample.channels.max(1) as usize;
    let mut data = sample.data.clone();
    let trimmed_frames = trim_tail(&mut data, ch, sample.sample_rate, TRIM_RATIO);
    let data = resample_linear(&data, ch, sample.sample_rate, device_rate);
    let data: Vec<f32> = match ch {
        1 => data.iter().flat_map(|&s| [s, s]).collect(),
        2 => data,
        _ => data.chunks_exact(ch).flat_map(|f| [f[0], f[1]]).collect(),
    };
    let total = data.len() / 2;
    let xfade = xfade_frames(total, device_rate);
    let loop_len = total.saturating_sub(xfade).max(1);
    let mut frames = vec![0.0f32; loop_len * 2];
    for i in 0..loop_len {
        for c in 0..2 {
            frames[i * 2 + c] = if i < xfade && loop_len + i < total {
                let (s, co) = (i as f32 / xfade as f32 * FRAC_PI_2).sin_cos();
                data[(loop_len + i) * 2 + c] * co + data[i * 2 + c] * s
            } else {
                data.get(i * 2 + c).copied().unwrap_or(0.0)
            };
        }
    }
    let gain = normalise_gain(&frames);
    for s in frames.iter_mut() {
        *s *= gain;
    }
    LoopSample { frames: frames.into_boxed_slice(), loop_len, xfade, trimmed_frames, gain }
}

/// A playing rain voice. Runs on the audio thread: no allocation; starting one
/// is an `Arc::clone`, dropping one only decrements the count.
pub struct LoopVoice {
    sample: Arc<LoopSample>,
    pos: usize,
    env_phase: f32,
    env_step: f32,
}

impl LoopVoice {
    /// Start at the loop head with an equal-power fade-in over `fade_secs`.
    pub fn start(sample: Arc<LoopSample>, rate: f32, fade_secs: f32) -> Self {
        Self { sample, pos: 0, env_phase: 0.0, env_step: 1.0 / (rate * fade_secs) }
    }

    /// Start at full level, no fade-in (offline rendering).
    pub fn full(sample: Arc<LoopSample>) -> Self {
        Self { sample, pos: 0, env_phase: 1.0, env_step: 0.0 }
    }

    /// Fade out from the current level, equal-power, over `fade_secs` from full.
    pub fn release(&mut self, rate: f32, fade_secs: f32) {
        self.env_step = -1.0 / (rate * fade_secs);
    }

    pub fn is_releasing(&self) -> bool {
        self.env_step < 0.0
    }

    pub fn level(&self) -> f32 {
        self.env_phase
    }

    pub fn finished(&self) -> bool {
        self.is_releasing() && self.env_phase <= 0.0
    }

    #[inline]
    pub fn next(&mut self) -> (f32, f32) {
        self.env_phase = (self.env_phase + self.env_step).clamp(0.0, 1.0);
        let g = (self.env_phase * FRAC_PI_2).sin();
        let i = self.pos * 2;
        let f = &self.sample.frames;
        let out = (f[i] * g, f[i + 1] * g);
        self.pos += 1;
        if self.pos == self.sample.loop_len {
            self.pos = 0;
        }
        out
    }
}

/// Render `loops` full passes of a baked loop as interleaved stereo.
pub fn render_loops(sample: &Arc<LoopSample>, loops: usize) -> Vec<f32> {
    let mut voice = LoopVoice::full(Arc::clone(sample));
    let mut out = Vec::with_capacity(sample.loop_len * loops * 2);
    for _ in 0..sample.loop_len * loops {
        let (l, r) = voice.next();
        out.push(l);
        out.push(r);
    }
    out
}

/// Seam metrics over rendered interleaved stereo with loop boundaries every `loop_len` frames.
pub struct SeamStats {
    /// Max sample-to-sample delta within 32 frames either side of each loop boundary.
    pub seam_delta: f32,
    /// Max sample-to-sample delta everywhere else.
    pub body_delta: f32,
    /// Smallest and median 50 ms RMS window.
    pub rms_min: f32,
    pub rms_median: f32,
}

pub fn seam_stats(out: &[f32], loop_len: usize, rate: u32) -> SeamStats {
    let frames = out.len() / 2;
    let near_seam = |n: usize| {
        let off = n % loop_len;
        off < 32 || off >= loop_len.saturating_sub(32)
    };
    let mut seam_delta = 0.0f32;
    let mut body_delta = 0.0f32;
    for n in 1..frames {
        for c in 0..2 {
            let d = (out[n * 2 + c] - out[(n - 1) * 2 + c]).abs();
            if n >= 32 && n + 32 < frames && near_seam(n) {
                seam_delta = seam_delta.max(d);
            } else if !near_seam(n) {
                body_delta = body_delta.max(d);
            }
        }
    }
    let win = (rate as usize / 20).max(1);
    let mut env: Vec<f32> = out
        .chunks_exact(win * 2)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    env.sort_by(|a, b| a.total_cmp(b));
    let rms_min = env.first().copied().unwrap_or(0.0);
    let rms_median = env.get(env.len() / 2).copied().unwrap_or(0.0);
    SeamStats { seam_delta, body_delta, rms_min, rms_median }
}

pub fn samples_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?.join("samples-rain");
        if dir.exists() { return Some(dir); }
    }
    let cwd = PathBuf::from("samples-rain");
    if cwd.exists() { return Some(cwd); }
    None
}

pub fn rain_file(rain_type: u8) -> Option<&'static str> {
    match rain_type {
        1 => Some("light-rain.wav"),
        2 => Some("calm-rain.wav"),
        3 => Some("heavy-rain.wav"),
        _ => None,
    }
}

pub fn load_sample(rain_type: u8) -> Option<Sample> {
    Sample::from_wav(&samples_dir()?.join(rain_file(rain_type)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    const RATE: u32 = 44100;

    fn white_noise(secs: f32, seed: u64) -> Vec<f32> {
        let mut rng = SmallRng::seed_from_u64(seed);
        (0..(RATE as f32 * secs) as usize * 2).map(|_| rng.gen_range(-0.5..0.5)).collect()
    }

    /// Stereo leaky random walk: smooth enough that its body sample-to-sample
    /// delta is small, so a seam discontinuity stands out as an outlier.
    fn smooth_noise(secs: f32, seed: u64) -> Vec<f32> {
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut x = [0.0f32; 2];
        let mut step = |rng: &mut SmallRng| {
            for v in x.iter_mut() {
                *v = *v * 0.995 + rng.gen_range(-0.02..0.02);
            }
            x
        };
        for _ in 0..5000 {
            step(&mut rng);
        }
        (0..(RATE as f32 * secs) as usize).flat_map(|_| step(&mut rng)).collect()
    }

    const SEEDS: [u64; 4] = [3, 7, 9, 11];

    fn render_stats(sample: &Sample, device_rate: u32) -> SeamStats {
        let looped = Arc::new(bake_loop(sample, device_rate));
        assert_eq!(looped.frames.len(), looped.loop_len * 2);
        seam_stats(&render_loops(&looped, 3), looped.loop_len, device_rate)
    }

    /// Click check. Needs smooth input: on white noise every delta is already large.
    fn assert_no_click(sample: &Sample, device_rate: u32) {
        let s = render_stats(sample, device_rate);
        assert!(
            s.seam_delta <= s.body_delta * 1.05,
            "seam delta {} > body max {} * 1.05", s.seam_delta, s.body_delta
        );
    }

    /// Pump check. Needs white input: smooth noise swings in level on its own at 50 ms.
    fn assert_no_pump(sample: &Sample, device_rate: u32) {
        let s = render_stats(sample, device_rate);
        let ratio = s.rms_min / s.rms_median;
        assert!(ratio >= 0.8, "50 ms RMS min/median {ratio} < 0.8 (pump)");
    }

    fn assert_seamless(seed: u64, device_rate: u32) {
        let smooth = Sample { data: smooth_noise(3.0, seed), sample_rate: RATE, channels: 2 };
        assert_no_click(&smooth, device_rate);
        let white = Sample { data: white_noise(3.0, seed), sample_rate: RATE, channels: 2 };
        assert_no_pump(&white, device_rate);
    }

    #[test]
    fn baked_loop_of_noise_has_no_click_or_pump_at_the_seam() {
        for seed in SEEDS {
            assert_seamless(seed, RATE);
        }
    }

    #[test]
    fn baked_loop_seams_join_consecutive_source_frames() {
        let data = smooth_noise(3.0, 5);
        let sample = Sample { data: data.clone(), sample_rate: RATE, channels: 2 };
        let looped = bake_loop(&sample, RATE);
        let (n, x, g) = (looped.loop_len, looped.xfade, looped.gain);
        let out = |frame: usize, c: usize| looped.frames[frame * 2 + c];
        let src = |frame: usize, c: usize| data[frame * 2 + c] * g;
        for c in 0..2 {
            // Wrap seam: out[n-1] = data[n-1] is followed by out[0] = data[n].
            assert!((out(0, c) - src(n, c)).abs() < 1e-6, "loop head is not data[loop_len]");
            assert!((out(n - 1, c) - src(n - 1, c)).abs() < 1e-6, "loop tail is not data[loop_len - 1]");
            // End of the crossfade: out[x] = data[x] untouched, out[x-1] ~ data[x-1].
            assert!((out(x, c) - src(x, c)).abs() < 1e-6, "body after the crossfade changed");
            assert!((out(x - 1, c) - src(x - 1, c)).abs() < 0.01, "crossfade does not end on the head");
        }
    }

    #[test]
    fn baked_loop_stays_seamless_when_resampled_to_48k() {
        for seed in SEEDS {
            assert_seamless(seed, 48000);
        }
        let sample = Sample { data: white_noise(3.0, 2), sample_rate: RATE, channels: 2 };
        let looped = bake_loop(&sample, 48000);
        let total = looped.loop_len + looped.xfade;
        let expected = (3.0 * 48000.0) as usize;
        // Trim may remove up to two 50 ms windows of the flat source.
        assert!(total <= expected && total + 2 * 2400 + 2 >= expected, "total {total} vs {expected}");
    }

    #[test]
    fn loop_len_is_total_minus_xfade() {
        let sample = Sample { data: white_noise(30.0, 3), sample_rate: RATE, channels: 2 };
        let looped = bake_loop(&sample, RATE);
        let total = sample.data.len() / 2 - looped.trimmed_frames;
        assert_eq!(looped.xfade, (total as f32 * 0.10) as usize, "30 s file: xfade is 10 %");
        assert_eq!(looped.loop_len, total - looped.xfade);
    }

    #[test]
    fn xfade_is_a_tenth_of_the_sample_clamped_to_one_to_five_seconds() {
        let r = RATE as usize;
        assert_eq!(xfade_frames(15 * r, RATE), 15 * r / 10);
        assert_eq!(xfade_frames(20 * r, RATE), 2 * r);
        assert_eq!(xfade_frames(5 * r, RATE), r, "short sample: 1 s minimum");
        assert_eq!(xfade_frames(80 * r, RATE), 5 * r, "long sample: 5 s maximum");
        assert_eq!(xfade_frames(r, RATE), r / 3, "below 3 s: at most a third");
    }

    #[test]
    fn loud_input_is_attenuated_to_the_target_rms() {
        // White noise in -0.5..0.5 has RMS about 0.29, well above -30 dBFS (0.032).
        let data = white_noise(3.0, 12);
        let looped = bake_loop(&Sample { data, sample_rate: RATE, channels: 2 }, RATE);
        let f = &looped.frames;
        let rms = (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt();
        let target = 10f32.powf(LOOP_RMS_DBFS / 20.0);
        assert!((rms - target).abs() < target * 1e-3, "rms {rms} vs target {target}");
        assert!(looped.gain < 1.0);
    }

    #[test]
    fn input_quieter_than_the_target_is_not_boosted() {
        // RMS about 0.006, below -30 dBFS.
        let data: Vec<f32> = white_noise(3.0, 13).iter().map(|s| s * 0.02).collect();
        let sample = Sample { data, sample_rate: RATE, channels: 2 };
        let looped = bake_loop(&sample, RATE);
        assert_eq!(looped.gain, 1.0);
        let n = looped.loop_len;
        assert_eq!(looped.frames[n * 2 - 2..], sample.data[n * 2 - 2..n * 2], "body sample changed");
    }

    #[test]
    fn silence_keeps_unity_gain() {
        let sample = Sample { data: vec![0.0; RATE as usize * 3 * 2], sample_rate: RATE, channels: 2 };
        assert_eq!(bake_loop(&sample, RATE).gain, 1.0);
    }

    #[test]
    fn trim_tail_removes_a_baked_fade_out() {
        let mut data = white_noise(4.0, 4);
        let frames = data.len() / 2;
        let fade = RATE as usize;
        for i in frames - fade..frames {
            let g = (frames - 1 - i) as f32 / fade as f32;
            data[i * 2] *= g;
            data[i * 2 + 1] *= g;
        }
        let sample = Sample { data: data.clone(), sample_rate: RATE, channels: 2 };

        let mut trimmed = data;
        let removed = trim_tail(&mut trimmed, 2, RATE, TRIM_RATIO);
        // A linear ramp drops below 0.7 of full level after 0.3 s, so at least 0.7 s goes.
        assert!(removed >= fade * 7 / 10, "removed only {removed} of {fade} faded frames");
        assert!(removed <= fade, "removed {removed}, more than the {fade} faded frames");

        assert_no_pump(&sample, RATE);
    }

    #[test]
    fn zero_sample_rate_does_not_panic_the_bake() {
        let data = white_noise(1.0, 8);
        assert_eq!(resample_linear(&data, 2, 0, 48000), data);
        let sample = Sample { data, sample_rate: 0, channels: 2 };
        assert!(bake_loop(&sample, 48000).loop_len > 0);
    }

    #[test]
    fn trim_tail_leaves_flat_material_almost_untouched() {
        let mut data = white_noise(3.0, 5);
        let removed = trim_tail(&mut data, 2, RATE, TRIM_RATIO);
        assert!(removed <= 2 * RATE as usize / 20, "removed {removed} frames from flat noise");
    }

    #[test]
    fn voice_release_fades_to_finished_with_equal_power() {
        let loop_len = 1000;
        let looped = Arc::new(LoopSample {
            frames: vec![1.0f32; loop_len * 2].into_boxed_slice(),
            loop_len,
            xfade: 1,
            trimmed_frames: 0,
            gain: 1.0,
        });
        let mut incoming = LoopVoice::start(Arc::clone(&looped), RATE as f32, 0.5);
        let mut outgoing = LoopVoice::full(Arc::clone(&looped));
        outgoing.release(RATE as f32, 0.5);
        for n in 0..RATE as usize / 2 {
            let (in_l, in_r) = incoming.next();
            let (out_l, out_r) = outgoing.next();
            for power in [in_l * in_l + out_l * out_l, in_r * in_r + out_r * out_r] {
                assert!((power - 1.0).abs() < 1e-3, "summed power {power} at frame {n}");
            }
        }
        outgoing.next();
        assert!(outgoing.finished());
        assert!(!incoming.finished());
    }
}
