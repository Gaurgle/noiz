use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::audio::AudioState;

const NOISE_NAMES: [&str; 8] = ["white", "pink", "brown", "focus", "sleep", "deep", "theta", "zen"];
const NOISE_DESCRIPTIONS: [&str; 8] = [
    "equal energy, bright",
    "natural, balanced",
    "deep, warm rumble",
    "pink+brown, 2Hz bin @80",
    "brown, 0.5Hz bin @60",
    "pink+brown, 1Hz bin @70",
    "brown, 4Hz theta @80",
    "deep, 0.3Hz bin @50",
];

// Infinity symbol frames — 2D breathing animation (5 states, 3 rows each)
const INF_FRAMES: [[&str; 3]; 5] = [
    ["       ·    ·       ",
     "      · ·  · ·      ",
     "       ·    ·       "],
    ["      ·      ·      ",
     "    ·  ·    ·  ·    ",
     "      ·      ·      "],
    ["     ·        ·     ",
     "   ·    ·  ·    ·   ",
     "     ·        ·     "],
    ["    ·          ·    ",
     "  ·    ··  ··    ·  ",
     "    ·          ·    "],
    ["   ·            ·   ",
     " ·     ·∞∞·     ·  ",
     "   ·            ·   "],
];

pub fn run_tui(state: Arc<AudioState>, timer_end: Option<Instant>) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let start_time = Instant::now();

    loop {
        let paused = state.paused.load(Ordering::Relaxed);
        let volume = *state.volume.lock().unwrap();
        let noise_idx = state.noise_type.load(Ordering::Relaxed) as usize;
        let binaural = *state.binaural_freq.lock().unwrap();
        let bin_base = *state.binaural_base.lock().unwrap();
        let bin_vol = *state.binaural_vol.lock().unwrap();
        let modulation = *state.modulation_depth.lock().unwrap();
        let noise_name = NOISE_NAMES.get(noise_idx).unwrap_or(&"?");
        let noise_desc = NOISE_DESCRIPTIONS.get(noise_idx).unwrap_or(&"");

        let timer_str = timer_end.map(|end| {
            let now = Instant::now();
            if now >= end {
                "done".to_string()
            } else {
                let remaining = end - now;
                let secs = remaining.as_secs();
                let min = secs / 60;
                let sec = secs % 60;
                format!("{min}:{sec:02}")
            }
        });

        if let Some(end) = timer_end {
            if Instant::now() >= end {
                *state.fade_out_duration.lock().unwrap() = 5.0;
                state.fade_out.store(true, Ordering::Relaxed);
                if paused {
                    break;
                }
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();

            let dim = Color::Rgb(100, 104, 125);
            let text = Color::Rgb(205, 214, 244);
            let blue = Color::Rgb(116, 199, 236);
            let green = Color::Rgb(166, 227, 161);
            let yellow = Color::Rgb(249, 226, 175);
            let peach = Color::Rgb(250, 179, 135);
            let mauve = Color::Rgb(203, 166, 247);
            let red = Color::Rgb(243, 139, 168);
            let surface = Color::Rgb(69, 71, 90);
            let subtle = Color::Rgb(55, 57, 75);

            let bar_len = 22;

            // Volume bar with thin blocks for smoother look
            let vol_filled = (volume * bar_len as f32) as usize;
            let vol_bar = format!("{}{}", "━".repeat(vol_filled), "╌".repeat(bar_len - vol_filled));

            // Modulation bar
            let mod_filled = (modulation / 0.20 * bar_len as f32) as usize;
            let mod_bar = format!("{}{}", "━".repeat(mod_filled.min(bar_len)), "╌".repeat(bar_len - mod_filled.min(bar_len)));

            let status_icon = if paused { "■" } else { "▸" };
            let status_color = if paused { yellow } else { green };

            let mut lines: Vec<Line> = Vec::new();

            // Header — logo + status on same visual block
            lines.push(Line::from(vec![
                Span::styled("  noiz", Style::default().fg(mauve).add_modifier(Modifier::BOLD)),
                Span::styled("  ", Style::default()),
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::styled(format!(" {noise_name}"), Style::default().fg(text)),
                Span::styled(format!("  {noise_desc}"), Style::default().fg(dim)),
            ]));

            lines.push(Line::from(Span::styled("  ─────────────────────────────────", Style::default().fg(subtle))));

            // Source selector — compact, clear active state
            for row_start in [0usize, 4] {
                let mut type_line: Vec<Span> = vec![Span::styled("  ", Style::default())];
                let row_end = (row_start + 4).min(NOISE_NAMES.len());
                for i in row_start..row_end {
                    let name = NOISE_NAMES[i];
                    let key = format!("{}", i + 1);
                    let is_active = i == noise_idx;
                    let sep = if i < row_end - 1 { "  " } else { "" };
                    if is_active {
                        type_line.push(Span::styled(key, Style::default().fg(blue).add_modifier(Modifier::BOLD)));
                        type_line.push(Span::styled(format!(" {name}{sep}"), Style::default().fg(text)));
                    } else {
                        type_line.push(Span::styled(key, Style::default().fg(surface)));
                        type_line.push(Span::styled(format!(" {name}{sep}"), Style::default().fg(surface)));
                    }
                }
                lines.push(Line::from(type_line));
            }

            lines.push(Line::from(""));

            // Volume
            lines.push(Line::from(vec![
                Span::styled("  vol ", Style::default().fg(dim)),
                Span::styled(&vol_bar, Style::default().fg(blue)),
                Span::styled(format!("  {:>3.0}%", volume * 100.0), Style::default().fg(dim)),
            ]));

            // Modulation
            lines.push(Line::from(vec![
                Span::styled("  mod ", Style::default().fg(dim)),
                Span::styled(&mod_bar, Style::default().fg(mauve)),
                Span::styled(format!("  {:>3.0}%", modulation * 100.0), Style::default().fg(dim)),
            ]));

            // Binaural + Timer row (conditional, on same conceptual level)
            if binaural > 0.0 || timer_str.is_some() {
                lines.push(Line::from(""));
                if binaural > 0.0 {
                    let bin_display = if binaural < 1.0 {
                        format!("{binaural:.1} Hz")
                    } else {
                        format!("{binaural:.0} Hz")
                    };
                    lines.push(Line::from(vec![
                        Span::styled("  bin ", Style::default().fg(dim)),
                        Span::styled(bin_display, Style::default().fg(peach)),
                        Span::styled(format!("  tone {bin_base:.0} Hz"), Style::default().fg(subtle)),
                        Span::styled(format!("  vol {:.0}%", bin_vol * 100.0), Style::default().fg(subtle)),
                    ]));
                }
                if let Some(ref t) = timer_str {
                    lines.push(Line::from(vec![
                        Span::styled("  tmr ", Style::default().fg(dim)),
                        Span::styled(t.clone(), Style::default().fg(yellow)),
                        Span::styled("  remaining", Style::default().fg(subtle)),
                    ]));
                }
            }

            // Visualizer — infinity symbol pulsing with binaural or LFO
            let elapsed = start_time.elapsed().as_secs_f32();
            let pulse_rate = if binaural > 0.0 {
                // Divide binaural freq to visually comfortable range (0.5-2 Hz)
                (binaural / 4.0).clamp(0.5, 2.0)
            } else {
                0.04 // match LFO rate
            };
            let phase = (elapsed * pulse_rate * std::f32::consts::TAU).sin();
            // Map sine (-1..1) to brightness (0.0..1.0)
            let brightness = (phase + 1.0) / 2.0;

            // Interpolate color between subtle and active
            let vis_color = if binaural > 0.0 {
                let r = (55.0 + brightness * (250.0 - 55.0)) as u8;
                let g = (57.0 + brightness * (179.0 - 57.0)) as u8;
                let b = (75.0 + brightness * (135.0 - 75.0)) as u8;
                Color::Rgb(r, g, b)
            } else {
                let r = (55.0 + brightness * (203.0 - 55.0)) as u8;
                let g = (57.0 + brightness * (166.0 - 57.0)) as u8;
                let b = (75.0 + brightness * (247.0 - 75.0)) as u8;
                Color::Rgb(r, g, b)
            };

            // Pick frame based on brightness for expansion effect
            let frame_idx = (brightness * (INF_FRAMES.len() - 1) as f32) as usize;
            let inf_frame = &INF_FRAMES[frame_idx.min(INF_FRAMES.len() - 1)];

            lines.push(Line::from(""));
            for row in inf_frame {
                lines.push(Line::from(Span::styled(
                    format!("  {row}"),
                    Style::default().fg(vis_color),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("  ─────────────────────────────────", Style::default().fg(subtle))));

            // Keybindings — grouped logically
            lines.push(Line::from(vec![
                Span::styled("  ↑↓", Style::default().fg(blue)),
                Span::styled(" vol  ", Style::default().fg(dim)),
                Span::styled("[]", Style::default().fg(mauve)),
                Span::styled(" mod  ", Style::default().fg(dim)),
                Span::styled("b", Style::default().fg(peach)),
                Span::styled("in  ", Style::default().fg(dim)),
                Span::styled("space", Style::default().fg(green)),
                Span::styled(" pause  ", Style::default().fg(dim)),
                Span::styled("q", Style::default().fg(red)),
                Span::styled("uit", Style::default().fg(dim)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  ←→", Style::default().fg(peach)),
                Span::styled(" hz   ", Style::default().fg(dim)),
                Span::styled("+-", Style::default().fg(peach)),
                Span::styled(" pitch  ", Style::default().fg(dim)),
                Span::styled("<>", Style::default().fg(peach)),
                Span::styled(" bin vol", Style::default().fg(dim)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(surface))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .padding(Padding::new(1, 1, 1, 0));

            let paragraph = Paragraph::new(lines).block(block);
            frame.render_widget(paragraph, area);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                let is_quit = matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));

                if is_quit {
                    state.fade_out.store(true, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(800));
                    break;
                }

                match key.code {
                    KeyCode::Char(' ') => {
                        let current = state.paused.load(Ordering::Relaxed);
                        state.paused.store(!current, Ordering::Relaxed);
                    }
                    KeyCode::Up => {
                        let mut vol = state.target_volume.lock().unwrap();
                        *vol = (*vol + 0.05).min(1.0);
                    }
                    KeyCode::Down => {
                        let mut vol = state.target_volume.lock().unwrap();
                        *vol = (*vol - 0.05).max(0.0);
                    }
                    KeyCode::Char(']') => {
                        let mut m = state.modulation_depth.lock().unwrap();
                        *m = (*m + 0.02).min(0.20);
                    }
                    KeyCode::Char('[') => {
                        let mut m = state.modulation_depth.lock().unwrap();
                        *m = (*m - 0.02).max(0.0);
                    }
                    KeyCode::Char('b') => {
                        let mut b = state.binaural_freq.lock().unwrap();
                        if *b == 0.0 {
                            *b = 4.0;
                        } else {
                            *b = 0.0;
                        }
                    }
                    KeyCode::Right => {
                        let mut b = state.binaural_freq.lock().unwrap();
                        if *b > 0.0 {
                            let step = if *b < 1.0 { 0.1 } else { 1.0 };
                            *b = (*b + step).min(20.0);
                        }
                    }
                    KeyCode::Left => {
                        let mut b = state.binaural_freq.lock().unwrap();
                        if *b > 0.0 {
                            let step = if *b <= 1.0 { 0.1 } else { 1.0 };
                            *b = ((*b - step) * 10.0).round() / 10.0; // avoid float drift
                            if *b < 0.1 { *b = 0.1; }
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        let mut base = state.binaural_base.lock().unwrap();
                        let step = if *base < 60.0 { 2.0 } else { 10.0 };
                        *base = (*base + step).min(300.0);
                    }
                    KeyCode::Char('-') => {
                        let mut base = state.binaural_base.lock().unwrap();
                        let step = if *base <= 60.0 { 2.0 } else { 10.0 };
                        *base = (*base - step).max(20.0);
                    }
                    KeyCode::Char('>') | KeyCode::Char('.') => {
                        let mut v = state.binaural_vol.lock().unwrap();
                        *v = (*v + 0.05).min(1.0);
                    }
                    KeyCode::Char('<') | KeyCode::Char(',') => {
                        let mut v = state.binaural_vol.lock().unwrap();
                        *v = (*v - 0.05).max(0.0);
                    }
                    KeyCode::Char('1') => {
                        state.noise_type.store(0, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('2') => {
                        state.noise_type.store(1, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('3') => {
                        state.noise_type.store(2, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('4') => {
                        state.noise_type.store(3, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('5') => {
                        state.noise_type.store(4, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('6') => {
                        state.noise_type.store(5, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('7') => {
                        state.noise_type.store(6, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('8') => {
                        state.noise_type.store(7, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}
