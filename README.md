# noiz

![Rust](https://img.shields.io/badge/Rust-stable-dea584?logo=rust)
![Platform](https://img.shields.io/badge/platform-macOS%20·%20Linux-lightgrey)
![License](https://img.shields.io/badge/License-MIT-green)
![Open Source](https://img.shields.io/badge/open%20source-%E2%9D%A4-c4a7e7)
![Audio](https://img.shields.io/badge/audio-white%20·%20pink%20·%20brown-89b4fa)
![Binaural](https://img.shields.io/badge/binaural-0.1--20%20Hz-fab387)

Terminal noise generator for focus and concentration.

Real-time stereo noise synthesis with smooth crossfading, subtle modulation, binaural beats, and a minimal TUI. No audio files — everything is generated on the fly.

## Install

```bash
git clone https://github.com/Gaurgle/noiz.git
cd noiz
cargo build --release
cp target/release/noiz ~/.local/bin/
```

## Usage

```bash
noiz                    # brown noise (default)
noiz white              # white noise
noiz pink               # pink noise
noiz focus              # preset: pink+brown + 2Hz binaural
noiz sleep              # preset: brown + 0.5Hz binaural
noiz deep               # preset: pink+brown + 1Hz binaural
noiz theta              # preset: brown + 4Hz theta binaural
noiz zen                # preset: deep + 0.3Hz binaural
noiz pink 45m           # timer — fade out after 45 minutes
noiz brown -v 0.7       # set volume (0.0–1.0)
noiz pink -b 4          # binaural beats at 4 Hz
noiz pink+brown         # mix noise types
```

## Keybindings

| Key | Action |
|-----|--------|
| `1`–`3` | Noise type: white, pink, brown |
| `4`–`8` | Presets: focus, sleep, deep, theta, zen |
| `↑` / `↓` | Volume up/down |
| `[` / `]` | Modulation depth up/down |
| `b` | Toggle binaural on/off |
| `←` / `→` | Binaural pitch split (0.1–20 Hz) |
| `+` / `-` | Binaural base tone (20–300 Hz) |
| `<` / `>` | Binaural volume |
| `space` | Pause/resume |
| `q` / `Esc` / `Ctrl+C` | Quit (750ms fade out) |

## Noise Types

| Type | Description |
|------|-------------|
| **white** | Equal energy across all frequencies — gain-balanced to match perceived loudness |
| **pink** | Energy drops 3 dB/octave — natural, balanced |
| **brown** | Energy drops 6 dB/octave — deep, dark, lowpass-filtered for extra warmth |

## Presets

| Preset | Mix | Split | Tone | Character |
|--------|-----|-------|------|-----------|
| **focus** | 80% pink + 20% brown | 2 Hz | 80 Hz | Slow, masks distractions |
| **sleep** | 10% pink + 90% brown | 0.5 Hz | 60 Hz | Very slow, deep |
| **deep** | 40% pink + 60% brown | 1 Hz | 70 Hz | Dark, full-bodied |
| **theta** | brown | 4 Hz | 80 Hz | Theta waves, meditation |
| **zen** | 30% pink + 70% brown | 0.3 Hz | 50 Hz | Extremely slow, deepest |

## Features

- **Stereo** — independent noise generators per channel with subtle LFO phase offset for width
- **Smooth transitions** — 2-second crossfade when switching noise types
- **Fade in/out** — 750ms fade on start and quit, 5s fade on timer expiry
- **Modulation** — adjustable slow LFO on volume (different phase per channel) to prevent static feel
- **Binaural beats** — discrete L/R sine tones with configurable pitch split (0.1–20 Hz), base tone (20–300 Hz), and volume. The perceived beat emerges from the frequency difference between ears
- **Visualizer** — 2D infinity symbol that breathes in sync with binaural modulation
- **Timer** — supports `30s`, `45m`, `1h` durations with automatic fade out

## Stack

Rust, [cpal](https://github.com/RustAudio/cpal), [ratatui](https://github.com/ratatui/ratatui), [crossterm](https://github.com/crossterm-rs/crossterm)
