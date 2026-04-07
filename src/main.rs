mod audio;
mod noise;
mod samples;
mod tui;

use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;

use audio::AudioState;

#[derive(Parser)]
#[command(name = "noiz", about = "Terminal noise generator for focus")]
struct Cli {
    /// Noise type: white, pink, brown
    #[arg(default_value = "brown")]
    source: String,

    /// Timer duration: 45m, 1h, 30s
    #[arg()]
    timer: Option<String>,

    /// Noise volume (0.0 - 1.0)
    #[arg(short, long, default_value = "0.3")]
    volume: f32,
}

fn parse_noise_type(s: &str) -> u8 {
    match s {
        "white" | "w" => 1,
        "pink" | "p" => 2,
        "brown" | "b" => 3,
        _ => 3,
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
