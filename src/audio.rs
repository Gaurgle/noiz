use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use rand::rngs::SmallRng;
use rand::SeedableRng;

use crate::noise::{NoiseMix, NoiseGen};

pub struct AudioState {
    pub volume: Mutex<f32>,
    pub target_volume: Mutex<f32>,
    pub paused: AtomicBool,
    pub noise_type: AtomicU8, // 0=white, 1=pink, 2=brown, 3=focus, 4=sleep, 5=deep
    pub pending_switch: AtomicBool,
    pub binaural_freq: Mutex<f32>, // 0.0 = off
    pub binaural_base: Mutex<f32>, // base tone frequency
    pub binaural_vol: Mutex<f32>,  // 0.0 - 1.0
    pub modulation_depth: Mutex<f32>, // 0.0 - 0.20
    pub fade_out: AtomicBool,
    pub fade_out_duration: Mutex<f32>, // seconds
}

impl AudioState {
    pub fn new(volume: f32) -> Self {
        Self {
            volume: Mutex::new(volume),
            target_volume: Mutex::new(volume),
            paused: AtomicBool::new(false),
            noise_type: AtomicU8::new(2), // default brown
            pending_switch: AtomicBool::new(false),
            binaural_freq: Mutex::new(0.0),
            binaural_base: Mutex::new(120.0),
            binaural_vol: Mutex::new(0.55),
            modulation_depth: Mutex::new(0.08), // default ±8%
            fade_out: AtomicBool::new(false),
            fade_out_duration: Mutex::new(0.75), // default 750ms
        }
    }
}

/// Returns (NoiseMix, Option<(binaural_freq, binaural_base)>)
fn mix_for_type(t: u8) -> (NoiseMix, Option<(f32, f32)>) {
    match t {
        0 => (NoiseMix::white(), None),
        1 => (NoiseMix::pink(), None),
        2 => (NoiseMix::brown(), None),
        3 => (NoiseMix { white: 0.0, pink: 0.8, brown: 0.2 }, Some((2.0, 80.0))),   // focus: slow bin, low tone
        4 => (NoiseMix { white: 0.0, pink: 0.1, brown: 0.9 }, Some((0.5, 60.0))),   // sleep: very slow, deep
        5 => (NoiseMix { white: 0.0, pink: 0.4, brown: 0.6 }, Some((1.0, 70.0))),   // deep: slow, low
        6 => (NoiseMix::brown(), Some((4.0, 80.0))),                                  // theta: 4Hz, low tone
        7 => (NoiseMix { white: 0.0, pink: 0.3, brown: 0.7 }, Some((0.3, 50.0))),   // zen: very slow, very deep
        _ => (NoiseMix::brown(), None),
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
    let mut gen = NoiseGen::new();
    let (init_mix, init_bin) = mix_for_type(state.noise_type.load(Ordering::Relaxed));
    if let Some((freq, base)) = init_bin {
        *state.binaural_freq.lock().unwrap() = freq;
        *state.binaural_base.lock().unwrap() = base;
    }
    let mut current_mix = init_mix;
    let mut target_mix = current_mix;
    let mut crossfade_progress = 1.0f32; // 1.0 = complete

    // Stereo LFO state — slightly different phase for L and R
    let mut lfo_phase_l: f32 = 0.0;
    let mut lfo_phase_r: f32 = 0.33; // offset for stereo width
    let lfo_rate = 0.04; // ~25 second cycle

    // Binaural oscillator phase
    let mut bin_phase_l: f32 = 0.0;
    let mut bin_phase_r: f32 = 0.0;
    // bin_base_freq read dynamically from state

    // Start at zero and ramp up (fade-in)
    let mut current_vol: f32 = 0.0;

    // Fade out state
    let mut fade_out_vol = 1.0f32;

    // Fade-in envelope
    let mut fade_in_vol = 0.0f32;

    let stream = device.build_output_stream(
        &stream_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let target_vol = *state.target_volume.lock().unwrap();
            let binaural = *state.binaural_freq.lock().unwrap();
            let lfo_depth = *state.modulation_depth.lock().unwrap();
            let paused = state.paused.load(Ordering::Relaxed);
            let fading = state.fade_out.load(Ordering::Relaxed);

            // Check for noise type switch
            if state.pending_switch.load(Ordering::Relaxed) {
                let (new_mix, new_bin) = mix_for_type(state.noise_type.load(Ordering::Relaxed));
                target_mix = new_mix;
                if let Some((freq, base)) = new_bin {
                    *state.binaural_freq.lock().unwrap() = freq;
                    *state.binaural_base.lock().unwrap() = base;
                } else {
                    *state.binaural_freq.lock().unwrap() = 0.0;
                }
                crossfade_progress = 0.0;
                state.pending_switch.store(false, Ordering::Relaxed);
            }

            let crossfade_speed = 1.0 / (sample_rate * 2.0); // 2 second crossfade

            for frame in data.chunks_mut(channels) {
                // Smooth volume ramping
                let vol_speed = 1.0 / (sample_rate * 0.1); // 100ms ramp
                if current_vol < target_vol {
                    current_vol = (current_vol + vol_speed).min(target_vol);
                } else if current_vol > target_vol {
                    current_vol = (current_vol - vol_speed).max(target_vol);
                }
                *state.volume.lock().unwrap() = current_vol;

                // Fade in (750ms)
                if fade_in_vol < 1.0 {
                    fade_in_vol = (fade_in_vol + 1.0 / (sample_rate * 0.75)).min(1.0);
                }

                // Fade out
                if fading {
                    let fade_dur = *state.fade_out_duration.lock().unwrap();
                    fade_out_vol -= 1.0 / (sample_rate * fade_dur);
                    if fade_out_vol <= 0.0 {
                        fade_out_vol = 0.0;
                        state.paused.store(true, Ordering::Relaxed);
                    }
                }

                if paused {
                    for sample in frame.iter_mut() {
                        *sample = 0.0;
                    }
                    continue;
                }

                // Crossfade between mixes
                if crossfade_progress < 1.0 {
                    crossfade_progress = (crossfade_progress + crossfade_speed).min(1.0);
                    if crossfade_progress >= 1.0 {
                        current_mix = target_mix;
                    }
                }

                let mix = if crossfade_progress < 1.0 {
                    NoiseMix::lerp(&current_mix, &target_mix, crossfade_progress)
                } else {
                    current_mix
                };

                let (l, r) = gen.sample(&mut rng, &mix);

                // Stereo LFO modulation
                let lfo_l = 1.0 + (lfo_phase_l * std::f32::consts::TAU).sin() * lfo_depth;
                let lfo_r = 1.0 + (lfo_phase_r * std::f32::consts::TAU).sin() * lfo_depth;
                lfo_phase_l = (lfo_phase_l + lfo_rate / sample_rate) % 1.0;
                lfo_phase_r = (lfo_phase_r + lfo_rate / sample_rate) % 1.0;

                let mut out_l = l * lfo_l;
                let mut out_r = r * lfo_r;

                // Binaural beat — low sine layered under noise
                if binaural > 0.0 {
                    let base = *state.binaural_base.lock().unwrap();
                    let freq_l = base;
                    let freq_r = base + binaural;
                    let bin_vol = *state.binaural_vol.lock().unwrap();
                    // Fundamental + soft octave harmonic for body
                    let sin_l = (bin_phase_l * std::f32::consts::TAU).sin();
                    let sin_r = (bin_phase_r * std::f32::consts::TAU).sin();
                    let harm_l = (bin_phase_l * 2.0 * std::f32::consts::TAU).sin() * 0.3;
                    let harm_r = (bin_phase_r * 2.0 * std::f32::consts::TAU).sin() * 0.3;
                    out_l += (sin_l + harm_l) * bin_vol;
                    out_r += (sin_r + harm_r) * bin_vol;
                    bin_phase_l = (bin_phase_l + freq_l / sample_rate) % 1.0;
                    bin_phase_r = (bin_phase_r + freq_r / sample_rate) % 1.0;
                }

                let vol = current_vol * fade_in_vol * fade_out_vol;
                let final_l = (out_l * vol).clamp(-1.0, 1.0);
                let final_r = (out_r * vol).clamp(-1.0, 1.0);

                // Output stereo or mono depending on channel count
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
