# noiz

![Rust](https://img.shields.io/badge/Rust-stable-dea584?logo=rust)
![Platform](https://img.shields.io/badge/platform-macOS%20·%20Linux-lightgrey)
![License](https://img.shields.io/badge/License-MIT-green)
![Open Source](https://img.shields.io/badge/open%20source-%E2%9D%A4-c4a7e7)
![Audio](https://img.shields.io/badge/audio-white%20·%20pink%20·%20brown-89b4fa)
![Binaural](https://img.shields.io/badge/binaural-delta%20·%20theta%20·%20alpha%20·%20beta%20·%20gamma-fab387)

Terminal noise generator for focus and concentration.

Real-time stereo noise synthesis with binaural brainwave presets, rain overlay, smooth crossfading, subtle modulation, and a minimal TUI. No audio files for noise — everything is generated on the fly.

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
noiz delta              # deep sleep — 2 Hz binaural
noiz theta              # meditation — 6 Hz binaural
noiz alpha              # relaxed focus — 10 Hz binaural
noiz beta               # active focus — 18 Hz binaural
noiz gamma              # deep concentration — 40 Hz binaural
noiz brown 45m          # timer — fade out after 45 minutes
noiz pink -v 0.7        # set volume (0.0–1.0)
```

## Keybindings

| Key | Action |
|-----|--------|
| `1`-`3` | Noise type: white, pink, brown |
| `4`-`8` | Binaural presets: delta, theta, alpha, beta, gamma |
| `↑` / `↓` | Volume (1% steps) |
| `[` / `]` | Modulation depth |
| `b` | Toggle binaural on/off |
| `←` / `→` | Binaural split frequency (0.1-40 Hz) |
| `+` / `-` | Binaural base tone (20-300 Hz) |
| `<` / `>` | Binaural volume |
| `r` | Cycle rain overlay: off → light → calm → heavy → off |
| `space` | Pause/resume (0.4s fade) |
| `q` / `Esc` / `Ctrl+C` | Quit (750ms fade out) |

## Noise Types

| Type | Description |
|------|-------------|
| **white** | Equal energy across all frequencies — gain-balanced for perceived loudness |
| **pink** | Energy drops 3 dB/octave — natural, balanced |
| **brown** | Energy drops 6 dB/octave — deep, dark, lowpass-filtered for warmth |

## Binaural Presets

Each preset pairs a noise mix with a binaural beat tuned to a specific brainwave band. The binaural effect is created by playing two discrete sine tones — one per ear — with a slight frequency difference. The brain perceives this difference as a rhythmic pulse at the target frequency.

**Requires headphones** — binaural beats do not work over speakers.

| Preset | Brainwave | Split | Tone | Noise Mix | Effect |
|--------|-----------|-------|------|-----------|--------|
| **delta** | 1-4 Hz | 2 Hz | 60 Hz | 10% pink + 90% brown | Deep sleep, healing |
| **theta** | 4-8 Hz | 6 Hz | 80 Hz | 30% pink + 70% brown | Meditation, creativity |
| **alpha** | 8-14 Hz | 10 Hz | 100 Hz | 70% pink + 30% brown | Relaxed focus, flow |
| **beta** | 14-30 Hz | 18 Hz | 120 Hz | 80% pink + 20% brown | Active focus, energy |
| **gamma** | 30-100 Hz | 40 Hz | 140 Hz | 100% pink | Deep concentration |

Binaural can also be toggled manually with `b` and fine-tuned with arrow keys, independent of presets.

## Rain Overlay

Press `r` to cycle through rain samples layered on top of any noise/binaural preset:

| Rain | Intensity | Animation |
|------|-----------|-----------|
| **light** | Gentle patter | 3 drops |
| **calm** | Steady rain | 6 drops |
| **heavy** | Downpour | 10 drops |

Rain samples are wav files loaded from `samples-rain/`. All transitions crossfade smoothly.

## Features

- **Stereo** — independent noise generators per channel with subtle LFO phase offset for width
- **Smooth transitions** — 2-second crossfade when switching noise types
- **Fade in/out** — 750ms fade on start and quit, 0.4s fade on pause, 5s fade on timer expiry
- **Modulation** — adjustable slow LFO on volume (different phase per channel) to prevent static feel
- **Binaural beats** — discrete L/R sine tones with octave harmonic, preset or manual control
- **Rain overlay** — looping wav samples with crossfade, layered on any preset
- **Visualizer** — infinity symbol breathing with binaural, rain drops animated by intensity
- **Timer** — supports `30s`, `45m`, `1h` durations with automatic fade out

## Stack

Rust, [cpal](https://github.com/RustAudio/cpal), [ratatui](https://github.com/ratatui/ratatui), [crossterm](https://github.com/crossterm-rs/crossterm), [hound](https://github.com/ruuda/hound)
