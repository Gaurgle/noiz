use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::audio::AudioState;

const NOISE_NAMES: [&str; 6] = ["white", "pink", "brown", "focus", "sleep", "deep"];

const LOGO: &str = r#"
  ░█▀█░█▀█░▀█▀░▀▀█
  ░█░█░█░█░░█░░▄▀░
  ░▀░▀░▀▀▀░▀▀▀░▀▀▀
"#;

pub fn run_tui(state: Arc<AudioState>, timer_end: Option<Instant>) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    loop {
        let paused = state.paused.load(Ordering::Relaxed);
        let volume = *state.volume.lock().unwrap();
        let noise_idx = state.noise_type.load(Ordering::Relaxed) as usize;
        let binaural = *state.binaural_freq.lock().unwrap();
        let modulation = *state.modulation_depth.lock().unwrap();
        let noise_name = NOISE_NAMES.get(noise_idx).unwrap_or(&"?");

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

        // Check if timer expired
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

            let dim = Color::Rgb(127, 132, 156);
            let text = Color::Rgb(205, 214, 244);
            let blue = Color::Rgb(116, 199, 236);
            let green = Color::Rgb(166, 227, 161);
            let yellow = Color::Rgb(249, 226, 175);
            let peach = Color::Rgb(250, 179, 135);
            let mauve = Color::Rgb(203, 166, 247);
            let red = Color::Rgb(243, 139, 168);
            let surface = Color::Rgb(69, 71, 90);

            // Volume bar
            let vol_bar_len = 20;
            let vol_filled = (volume * vol_bar_len as f32) as usize;
            let vol_bar = format!("{}{}", "█".repeat(vol_filled), "░".repeat(vol_bar_len - vol_filled));

            // Modulation bar
            let mod_bar_len = 20;
            let mod_filled = (modulation / 0.20 * mod_bar_len as f32) as usize;
            let mod_bar = format!("{}{}", "█".repeat(mod_filled.min(mod_bar_len)), "░".repeat(mod_bar_len - mod_filled.min(mod_bar_len)));

            let status_icon = if paused { "◆" } else { "▸" };
            let status_color = if paused { yellow } else { green };

            // Build lines
            let mut lines: Vec<Line> = Vec::new();

            // Logo
            for logo_line in LOGO.trim_start_matches('\n').lines() {
                lines.push(Line::from(Span::styled(logo_line, Style::default().fg(mauve))));
            }

            // Status
            lines.push(Line::from(vec![
                Span::styled(format!("  {status_icon} "), Style::default().fg(status_color)),
                Span::styled(format!("{noise_name}"), Style::default().fg(text).add_modifier(Modifier::BOLD)),
                Span::styled(if paused { "  paused" } else { "" }, Style::default().fg(dim)),
            ]));

            lines.push(Line::from(""));

            // Noise type selector
            let mut type_spans: Vec<Span> = vec![Span::styled("  src  ", Style::default().fg(dim))];
            for (i, name) in NOISE_NAMES.iter().enumerate() {
                let key = format!("{}", i + 1);
                let is_active = i == noise_idx;
                if is_active {
                    type_spans.push(Span::styled(key, Style::default().fg(blue).add_modifier(Modifier::BOLD)));
                    type_spans.push(Span::styled(format!("{} ", name), Style::default().fg(text)));
                } else {
                    type_spans.push(Span::styled(format!("{}{} ", key, name), Style::default().fg(surface)));
                }
            }
            lines.push(Line::from(type_spans));

            lines.push(Line::from(""));

            // Volume
            lines.push(Line::from(vec![
                Span::styled("  vol  ", Style::default().fg(dim)),
                Span::styled(&vol_bar, Style::default().fg(blue)),
                Span::styled(format!("  {:.0}%", volume * 100.0), Style::default().fg(dim)),
            ]));

            // Modulation
            lines.push(Line::from(vec![
                Span::styled("  mod  ", Style::default().fg(dim)),
                Span::styled(&mod_bar, Style::default().fg(mauve)),
                Span::styled(format!("  {:.0}%", modulation * 100.0), Style::default().fg(dim)),
            ]));

            // Binaural
            if binaural > 0.0 {
                lines.push(Line::from(vec![
                    Span::styled("  bin  ", Style::default().fg(dim)),
                    Span::styled(format!("{binaural:.0} Hz"), Style::default().fg(peach)),
                ]));
            }

            // Timer
            if let Some(ref t) = timer_str {
                lines.push(Line::from(vec![
                    Span::styled("  tmr  ", Style::default().fg(dim)),
                    Span::styled(t.clone(), Style::default().fg(yellow)),
                ]));
            }

            lines.push(Line::from(""));

            // Keybindings
            lines.push(Line::from(vec![
                Span::styled("  ↑↓", Style::default().fg(blue)),
                Span::styled(" vol  ", Style::default().fg(dim)),
                Span::styled("[]", Style::default().fg(mauve)),
                Span::styled(" mod  ", Style::default().fg(dim)),
                Span::styled("space", Style::default().fg(green)),
                Span::styled(" pause  ", Style::default().fg(dim)),
                Span::styled("q", Style::default().fg(red)),
                Span::styled("uit", Style::default().fg(dim)),
            ]));

            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(surface))
                .border_type(ratatui::widgets::BorderType::Rounded);

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
                    _ => {}
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}
