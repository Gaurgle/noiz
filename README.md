# noiz

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
noiz focus              # preset: 80% pink + 20% brown
noiz sleep              # preset: 10% pink + 90% brown
noiz deep               # preset: 40% pink + 60% brown
noiz pink 45m           # timer — fade out after 45 minutes
noiz brown -v 0.7       # set volume (0.0–1.0)
noiz pink -b 10         # binaural beats at 10 Hz
noiz pink+brown         # mix noise types
```

## Keybindings

| Key | Action |
|-----|--------|
| `1`–`6` | Switch noise type (white, pink, brown, focus, sleep, deep) |
| `↑` / `↓` | Volume up/down |
| `[` / `]` | Modulation depth up/down |
| `space` | Pause/resume |
| `q` / `Esc` / `Ctrl+C` | Quit (750ms fade out) |

## Noise Types

| Type | Description |
|------|-------------|
| **white** | Equal energy across all frequencies — balanced down to match perceived loudness |
| **pink** | Energy drops 3 dB/octave — natural, balanced, good default |
| **brown** | Energy drops 6 dB/octave — deep, rumbly, like distant thunder |

## Presets

| Preset | Mix | Character |
|--------|-----|-----------|
| **focus** | 80% pink + 20% brown | Present, masks distractions |
| **sleep** | 10% pink + 90% brown | Deep, dark, like a low hum |
| **deep** | 40% pink + 60% brown | Full-bodied, warm |

## Features

- **Stereo** — independent noise generators per channel with subtle LFO phase offset for width
- **Smooth transitions** — 2-second crossfade when switching noise types
- **Fade in/out** — 750ms fade on start and quit, 5s fade on timer expiry
- **Modulation** — adjustable slow LFO on volume (different phase per channel) to prevent static feel
- **Binaural beats** — low sine tone with configurable frequency split between L/R channels
- **Timer** — supports `30s`, `45m`, `1h` durations with automatic fade out

## Stack

Rust, [cpal](https://github.com/RustAudio/cpal), [ratatui](https://github.com/ratatui/ratatui), [crossterm](https://github.com/crossterm-rs/crossterm)
