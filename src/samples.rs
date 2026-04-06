use std::path::PathBuf;

pub struct Sample {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Sample {
    pub fn from_wav(path: &std::path::Path) -> Option<Self> {
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

pub struct SamplePlayer {
    data: Vec<f32>,
    channels: u16,
    sample_rate: u32,
    pos: usize,
    xfade_len: usize,
    pub fade_envelope: f32,
    pub fading_in: bool,
    pub fading_out: bool,
}

impl SamplePlayer {
    pub fn new(sample: Sample, crossfade_ms: f32) -> Self {
        let total = sample.data.len() / sample.channels as usize;
        let xfade = (sample.sample_rate as f32 * crossfade_ms / 1000.0) as usize;
        let xfade = xfade.min(total / 3);
        Self {
            data: sample.data,
            channels: sample.channels,
            sample_rate: sample.sample_rate,
            pos: 0,
            xfade_len: xfade,
            fade_envelope: 0.0,
            fading_in: true,
            fading_out: false,
        }
    }

    /// Create player from preloaded sample (clones data — no disk I/O)
    pub fn from_preloaded(sample: &Sample, crossfade_ms: f32) -> Self {
        let total = sample.data.len() / sample.channels as usize;
        let xfade = (sample.sample_rate as f32 * crossfade_ms / 1000.0) as usize;
        let xfade = xfade.min(total / 3);
        Self {
            data: sample.data.clone(),
            channels: sample.channels,
            sample_rate: sample.sample_rate,
            pos: 0,
            xfade_len: xfade,
            fade_envelope: 0.0,
            fading_in: true,
            fading_out: false,
        }
    }

    pub fn next_stereo(&mut self, target_rate: f32) -> (f32, f32) {
        let ch = self.channels as usize;
        let total = self.data.len() / ch;
        if total == 0 { return (0.0, 0.0); }

        // Smooth fade envelope (500ms fade in/out)
        let fade_speed = 1.0 / (target_rate * 0.5);
        if self.fading_in {
            self.fade_envelope = (self.fade_envelope + fade_speed).min(1.0);
            if self.fade_envelope >= 1.0 { self.fading_in = false; }
        }
        if self.fading_out {
            self.fade_envelope = (self.fade_envelope - fade_speed).max(0.0);
        }

        // Read at current position
        let frame = self.pos;
        let idx = frame * ch;
        let l = self.data[idx];
        let r = if ch > 1 { self.data[idx + 1] } else { l };

        // Crossfade zone: last xfade_len frames blend with first xfade_len frames
        let loop_end = total - self.xfade_len;
        let (out_l, out_r) = if frame >= loop_end && self.xfade_len > 0 {
            let into_xfade = frame - loop_end;
            let t = into_xfade as f32 / self.xfade_len as f32; // 0.0→1.0
            let begin_idx = into_xfade * ch;
            let bl = self.data[begin_idx];
            let br = if ch > 1 { self.data[begin_idx + 1] } else { bl };
            (l * (1.0 - t) + bl * t, r * (1.0 - t) + br * t)
        } else {
            (l, r)
        };

        // Advance and loop
        self.pos += 1;
        if self.pos >= total {
            // Jump to after the crossfade zone start (avoid re-playing the blended beginning)
            self.pos = self.xfade_len;
        }

        (out_l * self.fade_envelope, out_r * self.fade_envelope)
    }

    pub fn is_silent(&self) -> bool {
        self.fading_out && self.fade_envelope <= 0.0
    }
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

pub fn load_sample(rain_type: u8) -> Option<Sample> {
    let dir = samples_dir()?;
    let file = match rain_type {
        1 => "light-rain.wav",
        2 => "calm-rain.wav",
        3 => "heavy-rain.wav",
        _ => return None,
    };
    Sample::from_wav(&dir.join(file))
}
