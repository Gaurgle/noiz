mod audio;
mod noise;
mod samples;
mod tui;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;

use audio::AudioState;

#[derive(Parser)]
#[command(name = "noiz", version, about = "Terminal noise generator for focus")]
struct Cli {
    /// Noise type: white, pink, brown
    #[arg(default_value = "pink")]
    source: String,

    /// Timer duration: 45m, 1h, 30s
    #[arg()]
    timer: Option<String>,

    /// Noise volume (0.0 - 1.0)
    #[arg(short, long, default_value = "0.3")]
    volume: f32,

    /// Render baked rain loops to this wav file and exit (inspection tool)
    #[arg(long, hide = true, value_name = "FILE")]
    render_rain: Option<PathBuf>,

    /// Rain to render: light, calm, heavy, or a path to any wav
    #[arg(long, hide = true, default_value = "light", requires = "render_rain")]
    render_type: String,

    /// Number of full loops to render
    #[arg(long, hide = true, default_value_t = 3, requires = "render_rain")]
    render_loops: usize,

    /// Output rate to bake for, as a device would run (default: the file's rate)
    #[arg(long, hide = true, requires = "render_rain")]
    render_rate: Option<u32>,
}

fn render_rain(out: &Path, rain: &str, loops: usize, rate: Option<u32>) -> Result<(), String> {
    let path = match rain {
        "light" | "calm" | "heavy" => {
            let rain_type = match rain { "light" => 1, "calm" => 2, _ => 3 };
            let dir = samples::samples_dir().ok_or("samples-rain directory not found")?;
            dir.join(samples::rain_file(rain_type).ok_or("unknown rain type")?)
        }
        other => PathBuf::from(other),
    };
    let sample = samples::Sample::from_wav(&path)
        .ok_or_else(|| format!("cannot read wav {}", path.display()))?;
    let rate = rate.unwrap_or(sample.sample_rate);
    let looped = Arc::new(samples::bake_loop(&sample, rate));
    let rendered = samples::render_loops(&looped, loops.max(1));

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let write_err = |e: hound::Error| format!("cannot write {}: {e}", out.display());
    let mut writer = hound::WavWriter::create(out, spec).map_err(write_err)?;
    for s in &rendered {
        writer.write_sample(*s).map_err(write_err)?;
    }
    writer.finalize().map_err(write_err)?;

    let stats = samples::seam_stats(&rendered, looped.loop_len, rate);
    let secs = |frames: usize, r: u32| frames as f32 / r as f32;
    eprintln!(
        "{}: {} Hz -> {rate} Hz, trimmed {:.2} s, xfade {:.2} s, loop {:.2} s x {loops}, gain {:.3} ({:+.1} dB)",
        path.display(),
        sample.sample_rate,
        secs(looped.trimmed_frames, sample.sample_rate),
        secs(looped.xfade, rate),
        secs(looped.loop_len, rate),
        looped.gain,
        20.0 * looped.gain.log10(),
    );
    eprintln!(
        "seam delta {:.4} / body max {:.4} = {:.3} (pass <= 1.05); 50 ms RMS min {:.4} / median {:.4} = {:.3} (pass >= 0.8)",
        stats.seam_delta,
        stats.body_delta,
        stats.seam_delta / stats.body_delta,
        stats.rms_min,
        stats.rms_median,
        stats.rms_min / stats.rms_median,
    );
    eprintln!("wrote {}", out.display());
    Ok(())
}

fn parse_noise_type(s: &str) -> u8 {
    match s {
        "white" | "w" => 1,
        "pink" | "p" => 2,
        "brown" | "b" => 3,
        _ => 2,
    }
}

fn parse_duration(s: &str) -> Option<Duration> {
    let s = s.trim().to_lowercase();
    if let Some(mins) = s.strip_suffix('m') {
        mins.parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    } else if let Some(hours) = s.strip_suffix('h') {
        hours.parse::<u64>().ok().map(|h| Duration::from_secs(h * 3600))
    } else if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>().ok().map(Duration::from_secs)
    } else {
        s.parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
    }
}

fn main() {
    let cli = Cli::parse();
    if let Some(out) = &cli.render_rain {
        if let Err(e) = render_rain(out, &cli.render_type, cli.render_loops, cli.render_rate) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    let noise_type = parse_noise_type(&cli.source);
    let volume = cli.volume.clamp(0.0, 1.0);

    let state = Arc::new(AudioState::new(volume));
    state.noise_type.store(noise_type, std::sync::atomic::Ordering::Relaxed);

    let timer_end = cli.timer.as_ref().and_then(|t| {
        parse_duration(t).map(|d| Instant::now() + d)
    });

    let audio_state = Arc::clone(&state);
    let _stream = match audio::start_audio(audio_state) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = tui::run_tui(state, timer_end) {
        eprintln!("tui error: {e}");
    }
}
