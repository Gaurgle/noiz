use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::noise::{NoiseMix, NoiseGen};
use crate::samples;

pub struct AudioState {
    // Noise
    pub noise_type: AtomicU8,         // 0=white, 1=pink, 2=brown
    pub noise_vol: Mutex<f32>,        // 0.0-1.0
    pub target_noise_vol: Mutex<f32>,
    pub pending_switch: AtomicBool,

    // Binaural (completely independent)
    pub binaural_preset: AtomicU8,    // 0=off, 1=delta, 2=theta, 3=alpha, 4=beta, 5=gamma
    pub binaural_freq: Mutex<f32>,    // split frequency in Hz
    pub binaural_base: Mutex<f32>,    // carrier tone Hz
    pub binaural_vol: Mutex<f32>,     // 0.0-1.0
    pub binaural_pending: AtomicBool,

    // Rain (overlay)
    pub rain_type: AtomicU8,          // 0=off, 1=light, 2=calm, 3=heavy
    pub rain_vol: Mutex<f32>,         // 0.0-1.0
    pub rain_pending: AtomicBool,

    // Global
    pub paused: AtomicBool,
    pub modulation_depth: Mutex<f32>,
    pub fade_out: AtomicBool,
    pub fade_out_duration: Mutex<f32>,

    // Timer end signal
    pub timer_signal: AtomicBool,
    pub tone_click: AtomicBool,
    pub tone_tick: AtomicBool,
}

impl AudioState {
    pub fn new(noise_vol: f32) -> Self {
        Self {
            noise_type: AtomicU8::new(3), // brown (0=off, 1=white, 2=pink, 3=brown)
            noise_vol: Mutex::new(noise_vol),
            target_noise_vol: Mutex::new(noise_vol),
            pending_switch: AtomicBool::new(false),

            binaural_preset: AtomicU8::new(0), // off
            binaural_freq: Mutex::new(0.0),
            binaural_base: Mutex::new(140.0), // 300 Hz default carrier
            binaural_vol: Mutex::new(0.16),
            binaural_pending: AtomicBool::new(false),

            rain_type: AtomicU8::new(0),
            rain_vol: Mutex::new(0.35),
            rain_pending: AtomicBool::new(false),

            paused: AtomicBool::new(false),
            modulation_depth: Mutex::new(0.08),
            fade_out: AtomicBool::new(false),
            fade_out_duration: Mutex::new(0.65),

            timer_signal: AtomicBool::new(false),
            tone_click: AtomicBool::new(false),
            tone_tick: AtomicBool::new(false),
        }
    }
}

fn noise_mix(t: u8) -> NoiseMix {
    match t {
        0 => NoiseMix { white: 0.0, pink: 0.0, brown: 0.0 }, // off
        1 => NoiseMix::white(),
        2 => NoiseMix::pink(),
        3 => NoiseMix::brown(),
        _ => NoiseMix::brown(),
    }
}

/// Binaural presets — only split freq and carrier. No noise changes.
/// Binaural presets set split freq only. Carrier is user-adjustable (default 300 Hz).
fn binaural_preset(t: u8) -> Option<f32> {
    match t {
        1 => Some(2.0),   // delta
        2 => Some(6.0),   // theta
        3 => Some(10.0),  // alpha
        4 => Some(18.0),  // beta
        5 => Some(40.0),  // gamma
        _ => None,
    }
}

pub fn start_audio(state: Arc<AudioState>) -> Result<Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device()
        .ok_or("no audio output device found")?;
    let config = device.default_output_config()
        .map_err(|e| format!("failed to get audio config: {e}"))?;

    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    let stream_config: cpal::StreamConfig = config.into();

    let mut rng = SmallRng::from_entropy();
    let mut noise_gen = NoiseGen::new();

    // Each noise source has its own independent volume envelope
    let init_type = state.noise_type.load(Ordering::Relaxed);
    let mut white_target: f32 = if init_type == 1 { 1.0 } else { 0.0 };
    let mut pink_target: f32 = if init_type == 2 { 1.0 } else { 0.0 };
    let mut brown_target: f32 = if init_type == 3 { 1.0 } else { 0.0 };
    let mut white_vol: f32 = white_target;
    let mut pink_vol: f32 = pink_target;
    let mut brown_vol: f32 = brown_target;

    // Preload rain samples
    let rain_samples: [Option<samples::Sample>; 3] = [
        samples::load_sample(1),
        samples::load_sample(2),
        samples::load_sample(3),
    ];
    let mut rain_player: Option<samples::SamplePlayer> = None;
    let mut rain_outgoing: Option<samples::SamplePlayer> = None;

    // Stereo LFO
    let mut lfo_phase_l: f32 = 0.0;
    let mut lfo_phase_r: f32 = 0.33;
    let lfo_rate = 0.04;

    // Binaural — pure sine, completely discrete L/R
    let mut bin_phase_l: f32 = 0.0;
    let mut bin_phase_r: f32 = 0.0;
    let mut bin_fade: f32 = 0.0;
    let mut bin_target_split: f32 = 0.0;
    let mut bin_active_split: f32 = 0.0; // last non-zero split, used during fade-out

    // Envelopes
    let mut current_noise_vol: f32 = 0.0;
    let mut fade_in_vol = 0.0f32;
    let mut fade_out_vol = 1.0f32;
    let mut pause_vol = 1.0f32;

    // Timer end signal — morse "end": E(.) N(-.) D(-..)
    // Unit=130ms, tone=196Hz (G3), smooth 10ms ramps
    let sig_freq: f32 = 196.0;
    let sig_vol: f32 = 0.18;
    let sig_unit = (sample_rate * 0.13) as usize;
    let sig_ramp = (sample_rate * 0.01) as usize; // 10ms attack/release
    // Pattern: (tone, units) — gaps are silence segments
    let sig_pattern: [(bool, usize); 11] = [
        (true, 1), (false, 3),                              // E + gap
        (true, 3), (false, 1), (true, 1), (false, 3),       // N + gap
        (true, 3), (false, 1), (true, 1), (false, 1), (true, 1), // D
    ];
    // Precompute segment start offsets in samples
    let mut sig_offsets: [usize; 12] = [0; 12];
    for i in 0..11 {
        sig_offsets[i + 1] = sig_offsets[i] + sig_pattern[i].1 * sig_unit;
    }
    let sig_total = sig_offsets[11];
    let mut sig_active = false;
    let mut sig_sample: usize = 0;
    let mut sig_phase: f32 = 0.0;

    // Click tone — single dit on timer toggle
    let mut click_active = false;
    let mut click_sample: usize = 0;
    let mut click_phase: f32 = 0.0;
    let click_len = sig_unit; // 130ms, same as one dit

    // Tick tone — tiny, shorter and higher-pitched than the click, for
    // minute/second nudges on the timer row
    let tick_freq: f32 = 320.0;
    let tick_vol: f32 = 0.14;
    let tick_len = (sample_rate * 0.045) as usize; // 45ms
    let tick_ramp = (sample_rate * 0.005) as usize; // 5ms attack/release
    let mut tick_active = false;
    let mut tick_sample: usize = 0;
    let mut tick_phase: f32 = 0.0;

    let stream = device.build_output_stream(
        &stream_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let target_nvol = *state.target_noise_vol.lock().unwrap();
            let binaural_split = *state.binaural_freq.lock().unwrap();
            let binaural_base = *state.binaural_base.lock().unwrap();
            let bin_vol = *state.binaural_vol.lock().unwrap();
            let rain_vol = *state.rain_vol.lock().unwrap();
            let lfo_depth = *state.modulation_depth.lock().unwrap();
            let paused = state.paused.load(Ordering::Relaxed);
            let fading = state.fade_out.load(Ordering::Relaxed);

            // Noise type switch — just set targets, envelopes do the rest
            if state.pending_switch.load(Ordering::Relaxed) {
                let t = state.noise_type.load(Ordering::Relaxed);
                white_target = if t == 1 { 1.0 } else { 0.0 };
                pink_target = if t == 2 { 1.0 } else { 0.0 };
                brown_target = if t == 3 { 1.0 } else { 0.0 };
                state.pending_switch.store(false, Ordering::Relaxed);
            }

            // Binaural preset switch
            if state.binaural_pending.load(Ordering::Relaxed) {
                let preset = state.binaural_preset.load(Ordering::Relaxed);
                if let Some(freq) = binaural_preset(preset) {
                    bin_target_split = freq;
                    bin_active_split = freq;
                    *state.binaural_freq.lock().unwrap() = freq;
                } else {
                    bin_target_split = 0.0;
                    // keep bin_active_split for fade-out
                    *state.binaural_freq.lock().unwrap() = 0.0;
                }
                state.binaural_pending.store(false, Ordering::Relaxed);
            }

            // Rain toggle
            if state.rain_pending.load(Ordering::Relaxed) {
                let rt = state.rain_type.load(Ordering::Relaxed);
                if let Some(mut current) = rain_player.take() {
                    current.fading_out = true;
                    current.fading_in = false;
                    rain_outgoing = Some(current);
                }
                if rt > 0 {
                    let idx = (rt - 1) as usize;
                    if let Some(ref sample) = rain_samples[idx] {
                        rain_player = Some(samples::SamplePlayer::from_preloaded(sample, 2000.0));
                    }
                }
                state.rain_pending.store(false, Ordering::Relaxed);
            }

            if rain_outgoing.as_ref().map_or(false, |p| p.is_silent()) {
                rain_outgoing = None;
            }

            let ramp_speed = 1.0 / (sample_rate * 1.5); // 1.5s ramp per generator

            for frame in data.chunks_mut(channels) {
                // Noise master volume ramping
                let vol_speed = 1.0 / (sample_rate * 0.1);
                if current_noise_vol < target_nvol {
                    current_noise_vol = (current_noise_vol + vol_speed).min(target_nvol);
                } else if current_noise_vol > target_nvol {
                    current_noise_vol = (current_noise_vol - vol_speed).max(target_nvol);
                }
                *state.noise_vol.lock().unwrap() = current_noise_vol;

                // Independent per-source envelope ramps
                if white_vol < white_target { white_vol = (white_vol + ramp_speed).min(white_target); }
                else if white_vol > white_target { white_vol = (white_vol - ramp_speed).max(white_target); }
                if pink_vol < pink_target { pink_vol = (pink_vol + ramp_speed).min(pink_target); }
                else if pink_vol > pink_target { pink_vol = (pink_vol - ramp_speed).max(pink_target); }
                if brown_vol < brown_target { brown_vol = (brown_vol + ramp_speed).min(brown_target); }
                else if brown_vol > brown_target { brown_vol = (brown_vol - ramp_speed).max(brown_target); }

                // Fade in (750ms)
                if fade_in_vol < 1.0 {
                    fade_in_vol = (fade_in_vol + 1.0 / (sample_rate * 0.75)).min(1.0);
                }

                // Fade out (quit/timer)
                if fading {
                    let fade_dur = *state.fade_out_duration.lock().unwrap();
                    fade_out_vol -= 1.0 / (sample_rate * fade_dur);
                    if fade_out_vol <= 0.0 {
                        fade_out_vol = 0.0;
                        if !sig_active {
                            state.paused.store(true, Ordering::Relaxed);
                        }
                    }
                } else if fade_out_vol < 1.0 {
                    // Recovering from fade-out (timer ended, user resumed)
                    fade_out_vol = (fade_out_vol + 1.0 / (sample_rate * 0.75)).min(1.0);
                }

                // Start timer signal when flag is set (driven by TUI 2s delay)
                if state.timer_signal.load(Ordering::Relaxed) && !sig_active {
                    sig_active = true;
                    sig_sample = 0;
                    sig_phase = 0.0;
                    state.timer_signal.store(false, Ordering::Relaxed);
                }

                // Click tone on timer toggle
                if state.tone_click.load(Ordering::Relaxed) {
                    click_active = true;
                    click_sample = 0;
                    click_phase = 0.0;
                    state.tone_click.store(false, Ordering::Relaxed);
                }

                // Tick tone on timer minute/second nudge
                if state.tone_tick.load(Ordering::Relaxed) {
                    tick_active = true;
                    tick_sample = 0;
                    tick_phase = 0.0;
                    state.tone_tick.store(false, Ordering::Relaxed);
                }

                // Pause fade (0.4s)
                let pause_speed = 1.0 / (sample_rate * 0.4);
                if paused {
                    pause_vol = (pause_vol - pause_speed).max(0.0);
                } else {
                    pause_vol = (pause_vol + pause_speed).min(1.0);
                }

                // Timer end signal (independent of main envelopes)
                let mut sig_out = 0.0f32;
                if sig_active {
                    // Find which segment we're in
                    let mut seg_idx = 0;
                    for i in 0..11 {
                        if sig_sample < sig_offsets[i + 1] { seg_idx = i; break; }
                    }
                    let (is_tone, units) = sig_pattern[seg_idx];
                    if is_tone {
                        let seg_len = units * sig_unit;
                        let pos_in_seg = sig_sample - sig_offsets[seg_idx];
                        // Smooth ramp envelope to avoid clicks
                        let ramp_env = if pos_in_seg < sig_ramp {
                            pos_in_seg as f32 / sig_ramp as f32
                        } else if pos_in_seg >= seg_len - sig_ramp {
                            (seg_len - 1 - pos_in_seg) as f32 / sig_ramp as f32
                        } else {
                            1.0
                        };
                        sig_out = (sig_phase * std::f32::consts::TAU).sin() * sig_vol * ramp_env;
                        sig_phase = (sig_phase + sig_freq / sample_rate) % 1.0;
                    }
                    sig_sample += 1;
                    if sig_sample >= sig_total {
                        sig_active = false;
                        state.paused.store(true, Ordering::Relaxed);
                    }
                }

                // Click tone (single dit)
                if click_active {
                    let ramp_env = if click_sample < sig_ramp {
                        click_sample as f32 / sig_ramp as f32
                    } else if click_sample >= click_len - sig_ramp {
                        (click_len - 1 - click_sample) as f32 / sig_ramp as f32
                    } else {
                        1.0
                    };
                    sig_out += (click_phase * std::f32::consts::TAU).sin() * sig_vol * ramp_env;
                    click_phase = (click_phase + sig_freq / sample_rate) % 1.0;
                    click_sample += 1;
                    if click_sample >= click_len { click_active = false; }
                }

                // Tick tone (tiny nudge)
                if tick_active {
                    let ramp_env = if tick_sample < tick_ramp {
                        tick_sample as f32 / tick_ramp as f32
                    } else if tick_sample >= tick_len - tick_ramp {
                        (tick_len - 1 - tick_sample) as f32 / tick_ramp as f32
                    } else {
                        1.0
                    };
                    sig_out += (tick_phase * std::f32::consts::TAU).sin() * tick_vol * ramp_env;
                    tick_phase = (tick_phase + tick_freq / sample_rate) % 1.0;
                    tick_sample += 1;
                    if tick_sample >= tick_len { tick_active = false; }
                }

                if pause_vol <= 0.0 && !sig_active && !click_active && !tick_active && sig_out == 0.0 {
                    for sample in frame.iter_mut() { *sample = 0.0; }
                    continue;
                }

                // Generate noise — all generators run, mixed by their envelopes
                let mix = NoiseMix { white: white_vol, pink: pink_vol, brown: brown_vol };
                let (nl, nr) = noise_gen.sample(&mut rng, &mix);

                // LFO modulation on noise
                let lfo_l = 1.0 + (lfo_phase_l * std::f32::consts::TAU).sin() * lfo_depth;
                let lfo_r = 1.0 + (lfo_phase_r * std::f32::consts::TAU).sin() * lfo_depth;
                lfo_phase_l = (lfo_phase_l + lfo_rate / sample_rate) % 1.0;
                lfo_phase_r = (lfo_phase_r + lfo_rate / sample_rate) % 1.0;

                let mut out_l = nl * lfo_l * current_noise_vol;
                let mut out_r = nr * lfo_r * current_noise_vol;

                // Rain — own volume
                if let Some(ref mut player) = rain_player {
                    let (rl, rr) = player.next_stereo(sample_rate);
                    out_l += rl * rain_vol;
                    out_r += rr * rain_vol;
                }
                if let Some(ref mut player) = rain_outgoing {
                    let (rl, rr) = player.next_stereo(sample_rate);
                    out_l += rl * rain_vol;
                    out_r += rr * rain_vol;
                }

                // Binaural fade envelope (400ms)
                let bin_fade_speed = 1.0 / (sample_rate * 0.4);
                if bin_target_split > 0.0 {
                    bin_fade = (bin_fade + bin_fade_speed).min(1.0);
                } else {
                    bin_fade = (bin_fade - bin_fade_speed).max(0.0);
                }

                // Binaural — pure sine, discrete per channel
                if bin_fade > 0.0 {
                    let freq_l = binaural_base;
                    let freq_r = binaural_base + bin_active_split;
                    out_l += (bin_phase_l * std::f32::consts::TAU).sin() * bin_vol * bin_fade;
                    out_r += (bin_phase_r * std::f32::consts::TAU).sin() * bin_vol * bin_fade;
                    bin_phase_l = (bin_phase_l + freq_l / sample_rate) % 1.0;
                    bin_phase_r = (bin_phase_r + freq_r / sample_rate) % 1.0;
                }

                // Global envelope
                let envelope = fade_in_vol * fade_out_vol * pause_vol;
                let final_l = (out_l * envelope + sig_out).clamp(-1.0, 1.0);
                let final_r = (out_r * envelope + sig_out).clamp(-1.0, 1.0);

                match channels {
                    1 => frame[0] = (final_l + final_r) * 0.5,
                    _ => {
                        frame[0] = final_l;
                        if channels > 1 { frame[1] = final_r; }
                        for ch in frame.iter_mut().skip(2) { *ch = 0.0; }
                    }
                }
            }
        },
        |err| eprintln!("audio error: {err}"),
        None,
    ).map_err(|e| format!("failed to build audio stream: {e}"))?;

    stream.play().map_err(|e| format!("failed to play stream: {e}"))?;
    Ok(stream)
}
