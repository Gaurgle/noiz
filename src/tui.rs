use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::audio::AudioState;

const NOISE_NAMES: [&str; 4] = ["off", "white", "pink", "brown"];
const BIN_NAMES: [&str; 6] = ["off", "delta", "theta", "alpha", "beta", "gamma"];
const RAIN_NAMES: [&str; 4] = ["off", "light", "calm", "heavy"];

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

// Navigation rows
const ROW_NOISE: usize = 0;
const ROW_BIN: usize = 1;
const ROW_RAIN: usize = 2;
const ROW_SEP: usize = 3; // separator, not selectable
const ROW_NOISE_VOL: usize = 4;
const ROW_BIN_VOL: usize = 5;
const ROW_RAIN_VOL: usize = 6;
const ROW_BIN_CARRIER: usize = 7;
const ROW_MOD: usize = 8;
const ROW_COUNT: usize = 9;

fn row_selectable(r: usize) -> bool {
    r != ROW_SEP
}

fn next_row(current: usize, dir: i32) -> usize {
    let mut r = current;
    loop {
        r = ((r as i32 + dir).rem_euclid(ROW_COUNT as i32)) as usize;
        if row_selectable(r) { return r; }
    }
}

pub fn run_tui(state: Arc<AudioState>, timer_end: Option<Instant>) -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    terminal.clear()?;
    let mut anim_time: f32 = 0.0;
    let mut last_frame = Instant::now();
    let mut selected_row: usize = ROW_NOISE;

    loop {
        let paused = state.paused.load(Ordering::Relaxed);
        let noise_vol = *state.noise_vol.lock().unwrap();
        let noise_idx = state.noise_type.load(Ordering::Relaxed) as usize;
        let bin_preset = state.binaural_preset.load(Ordering::Relaxed) as usize; // 0=off, 1-5
        let binaural = *state.binaural_freq.lock().unwrap();
        let bin_base = *state.binaural_base.lock().unwrap();
        let bin_vol = *state.binaural_vol.lock().unwrap();
        let rain_type = state.rain_type.load(Ordering::Relaxed) as usize; // 0=off, 1-3
        let rain_vol = *state.rain_vol.lock().unwrap();
        let modulation = *state.modulation_depth.lock().unwrap();

        let timer_str = timer_end.map(|end| {
            let now = Instant::now();
            if now >= end { "done".to_string() }
            else { let s = (end - now).as_secs(); format!("{}:{:02}", s / 60, s % 60) }
        });

        if let Some(end) = timer_end {
            if Instant::now() >= end {
                *state.fade_out_duration.lock().unwrap() = 5.0;
                state.fade_out.store(true, Ordering::Relaxed);
                if paused { break; }
            }
        }

        terminal.draw(|frame| {
            let area = frame.area();
            let dim = Color::Rgb(100, 104, 125);
            let text = Color::Rgb(205, 214, 244);
            let c_noise = Color::Rgb(250, 179, 135);  // peach
            let c_bin = Color::Rgb(203, 166, 247);     // mauve
            let c_rain = Color::Rgb(116, 199, 236);    // blue
            let green = Color::Rgb(166, 227, 161);
            let yellow = Color::Rgb(249, 226, 175);
            let red = Color::Rgb(243, 139, 168);
            let surface = Color::Rgb(69, 71, 90);
            let subtle = Color::Rgb(55, 57, 75);

            let status_icon = if paused { "■" } else { "▸" };
            let status_color = if paused { yellow } else { green };
            let noise_name = NOISE_NAMES.get(noise_idx).unwrap_or(&"?");
            let noise_active = noise_idx > 0;
            let bin_active = bin_preset > 0;
            let rain_active = rain_type > 0;

            let mut lines: Vec<Line> = Vec::new();

            // Header
            lines.push(Line::from(vec![
                Span::styled("  noiz", Style::default().fg(c_bin).add_modifier(Modifier::BOLD)),
                Span::styled("  ", Style::default()),
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::styled(if paused { " paused" } else { "" }, Style::default().fg(dim)),
            ]));
            lines.push(Line::from(Span::styled("  ─────────────────────────────────────", Style::default().fg(subtle))));

            // Dimmed versions of source colors for sub-controls (tone, mod)
            let c_noise_dim = Color::Rgb(180, 130, 100);
            let c_bin_dim = Color::Rgb(150, 120, 180);
            let c_rain_dim = Color::Rgb(85, 140, 175);

            // Helper: bracket label — brackets always in source color
            let label = |name: &str, row: usize, color: Color, active: bool| -> Vec<Span<'static>> {
                let text_c = if active { color } else { surface };
                if selected_row == row {
                    vec![
                        Span::styled(" [".to_string(), Style::default().fg(color)),
                        Span::styled(name.to_string(), Style::default().fg(text_c)),
                        Span::styled("] ".to_string(), Style::default().fg(color)),
                    ]
                } else {
                    vec![
                        Span::styled(format!("  {name}  "), Style::default().fg(text_c)),
                    ]
                }
            };

            // Helper for source option rows
            let source_row = |items: &[&str], active_idx: usize, is_on: bool| -> Vec<Span<'static>> {
                let mut spans = Vec::new();
                for (i, name) in items.iter().enumerate() {
                    let is_selected = i == active_idx;
                    let sep = if i < items.len() - 1 { " " } else { "" };
                    let color = if is_selected {
                        text // active selection always bright, whether "off" or a preset
                    } else {
                        surface
                    };
                    spans.push(Span::styled(format!("{name}{sep}"), Style::default().fg(color)));
                }
                spans
            };

            // Noise source row
            let mut noise_line: Vec<Span> = label("noise", ROW_NOISE, c_noise, noise_active);
            noise_line.extend(source_row(&NOISE_NAMES, noise_idx, noise_active));
            lines.push(Line::from(noise_line));

            // Binaural source row
            let mut bin_line: Vec<Span> = label("bin", ROW_BIN, c_bin, bin_active);
            bin_line.push(Span::raw("  "));
            bin_line.extend(source_row(&BIN_NAMES, bin_preset, bin_active));
            lines.push(Line::from(bin_line));

            // Rain source row
            let mut rain_line: Vec<Span> = label("rain", ROW_RAIN, c_rain, rain_active);
            rain_line.push(Span::raw(" "));
            rain_line.extend(source_row(&RAIN_NAMES, rain_type, rain_active));
            lines.push(Line::from(rain_line));

            // Separator
            lines.push(Line::from(""));

            // Volume/control bars
            let bar_len = 20;

            let make_bar = |val: f32, color: Color, active: bool| -> String {
                let c = if active { color } else { surface };
                let _ = c;
                let filled = (val * bar_len as f32) as usize;
                format!("{}{}", "━".repeat(filled), "╌".repeat(bar_len - filled))
            };

            // Noise vol
            let nv_bar = make_bar(noise_vol, c_noise, noise_active);
            let mut nv_line = label("noise", ROW_NOISE_VOL, c_noise, noise_active);
            nv_line.push(Span::styled(&nv_bar, Style::default().fg(if noise_active { c_noise } else { surface })));
            nv_line.push(Span::styled(format!(" {:>3.0}%", noise_vol * 100.0), Style::default().fg(dim)));
            lines.push(Line::from(nv_line));

            // Bin vol
            let bv_bar = make_bar(bin_vol, c_bin, bin_active);
            let mut bv_line = label("bin", ROW_BIN_VOL, c_bin, bin_active);
            bv_line.push(Span::raw("  "));
            bv_line.push(Span::styled(&bv_bar, Style::default().fg(if bin_active { c_bin } else { surface })));
            bv_line.push(Span::styled(format!(" {:>3.0}%", bin_vol * 100.0), Style::default().fg(dim)));
            lines.push(Line::from(bv_line));

            // Rain vol
            let rv_bar = make_bar(rain_vol, c_rain, rain_active);
            let mut rv_line = label("rain", ROW_RAIN_VOL, c_rain, rain_active);
            rv_line.push(Span::raw(" "));
            rv_line.push(Span::styled(&rv_bar, Style::default().fg(if rain_active { c_rain } else { surface })));
            rv_line.push(Span::styled(format!(" {:>3.0}%", rain_vol * 100.0), Style::default().fg(dim)));
            lines.push(Line::from(rv_line));

            // Bin carrier — dimmed bin color
            let carrier_bar_val = ((bin_base - 40.0) / 360.0).clamp(0.0, 1.0);
            let cv_bar = make_bar(carrier_bar_val, c_bin_dim, bin_active);
            let mut cv_line = label("tone", ROW_BIN_CARRIER, c_bin_dim, bin_active);
            cv_line.push(Span::raw(" "));
            cv_line.push(Span::styled(&cv_bar, Style::default().fg(if bin_active { c_bin_dim } else { surface })));
            cv_line.push(Span::styled(format!(" {:>3.0}Hz", bin_base), Style::default().fg(dim)));
            lines.push(Line::from(cv_line));

            // Modulation — dimmed noise color
            let mod_bar = make_bar(modulation / 0.20, c_noise_dim, true);
            let mut mod_line = label("mod", ROW_MOD, c_noise_dim, true);
            mod_line.push(Span::raw("  "));
            mod_line.push(Span::styled(&mod_bar, Style::default().fg(c_noise_dim)));
            mod_line.push(Span::styled(format!(" {:>3.0}%", modulation * 100.0), Style::default().fg(dim)));
            lines.push(Line::from(mod_line));

            // Contextual info — relevant to what we're navigating
            let info_text: Option<String> = match selected_row {
                ROW_NOISE | ROW_NOISE_VOL | ROW_MOD => {
                    let n = NOISE_NAMES.get(noise_idx).unwrap_or(&"?");
                    Some(format!("  {n}  vol {:.0}%  mod {:.0}%", noise_vol * 100.0, modulation * 100.0))
                }
                ROW_BIN | ROW_BIN_VOL | ROW_BIN_CARRIER => {
                    if binaural > 0.0 {
                        let band = if binaural <= 4.0 { "delta" }
                            else if binaural <= 8.0 { "theta" }
                            else if binaural <= 14.0 { "alpha" }
                            else if binaural <= 30.0 { "beta" }
                            else { "gamma" };
                        let disp = if binaural < 1.0 { format!("{binaural:.1}") } else { format!("{binaural:.0}") };
                        Some(format!("  {disp} Hz {band}  carrier {bin_base:.0} Hz  vol {:.0}%", bin_vol * 100.0))
                    } else {
                        Some("  binaural off".to_string())
                    }
                }
                ROW_RAIN | ROW_RAIN_VOL => {
                    let r = RAIN_NAMES.get(rain_type).unwrap_or(&"?");
                    Some(format!("  {r}  vol {:.0}%", rain_vol * 100.0))
                }
                _ => None,
            };
            if let Some(info) = info_text {
                lines.push(Line::from(Span::styled(info, Style::default().fg(dim))));
            }

            // Timer
            if let Some(ref t) = timer_str {
                lines.push(Line::from(vec![
                    Span::styled("  tmr ", Style::default().fg(dim)),
                    Span::styled(t.clone(), Style::default().fg(yellow)),
                ]));
            }

            // Visualizer + rain animation
            let now_inst = Instant::now();
            if !paused { anim_time += (now_inst - last_frame).as_secs_f32(); }
            last_frame = now_inst;
            let elapsed = anim_time;

            let pulse_rate = if binaural > 0.0 { (binaural / 4.0).clamp(0.3, 2.0) } else { 0.04 };
            let phase = (elapsed * pulse_rate * std::f32::consts::TAU).sin();
            let brightness = (phase + 1.0) / 2.0;

            let vis_color = if bin_active {
                Color::Rgb(
                    (55.0 + brightness * 148.0) as u8,
                    (57.0 + brightness * 109.0) as u8,
                    (75.0 + brightness * 172.0) as u8,
                )
            } else {
                Color::Rgb(
                    (55.0 + brightness * 100.0) as u8,
                    (57.0 + brightness * 70.0) as u8,
                    (75.0 + brightness * 50.0) as u8,
                )
            };

            let frame_idx = (brightness * (INF_FRAMES.len() - 1) as f32) as usize;
            let inf_frame = &INF_FRAMES[frame_idx.min(INF_FRAMES.len() - 1)];

            let rain_width = 14;
            let rain_height = 3;
            let rain_light = Color::Rgb(100, 170, 220);
            let rain_dim_c = Color::Rgb(50, 90, 130);

            let all_drops: &[(usize, f32, f32)] = &[
                (1, 1.1, 0.0), (5, 0.8, 0.4), (10, 1.3, 0.7),
                (3, 0.9, 0.2), (8, 1.4, 0.5), (12, 0.7, 0.9),
                (0, 1.2, 0.1), (4, 1.6, 0.3), (7, 0.6, 0.6), (11, 1.0, 0.8),
            ];
            let drop_count = match rain_type { 1 => 3, 2 => 6, 3 => 10, _ => 0 };

            lines.push(Line::from(""));
            for row_idx in 0..rain_height {
                let mut spans = vec![
                    Span::styled(format!("  {}", inf_frame[row_idx]), Style::default().fg(vis_color)),
                ];
                if drop_count > 0 {
                    let mut rain_chars = vec![(' ', 0u8); rain_width];
                    for (i, &(col, speed, offset)) in all_drops.iter().take(drop_count).enumerate() {
                        let ps = offset + (i as f32 * 1.618);
                        let dp = (elapsed * speed + ps) % 1.0;
                        let dr = (dp * (rain_height as f32 + 1.0)) as usize;
                        if col < rain_width {
                            if dr == row_idx { rain_chars[col] = ('|', 2); }
                            else if dr == row_idx + 1 && rain_chars[col].1 < 1 { rain_chars[col] = ('·', 1); }
                        }
                    }
                    spans.push(Span::raw("  "));
                    for &(ch, _) in &rain_chars {
                        match ch {
                            '|' => spans.push(Span::styled("|", Style::default().fg(rain_light))),
                            '·' => spans.push(Span::styled("·", Style::default().fg(rain_dim_c))),
                            _ => spans.push(Span::raw(" ")),
                        }
                    }
                }
                lines.push(Line::from(spans));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("  ─────────────────────────────────────", Style::default().fg(subtle))));

            lines.push(Line::from(vec![
                Span::styled("  ↑↓", Style::default().fg(text)),
                Span::styled(" select  ", Style::default().fg(dim)),
                Span::styled("←→", Style::default().fg(text)),
                Span::styled(" adjust  ", Style::default().fg(dim)),
                Span::styled("n", Style::default().fg(c_noise)),
                Span::styled("oise ", Style::default().fg(dim)),
                Span::styled("b", Style::default().fg(c_bin)),
                Span::styled("in ", Style::default().fg(dim)),
                Span::styled("r", Style::default().fg(c_rain)),
                Span::styled("ain  ", Style::default().fg(dim)),
                Span::styled("space", Style::default().fg(green)),
                Span::styled(" pause  ", Style::default().fg(dim)),
                Span::styled("q", Style::default().fg(red)),
                Span::styled("uit", Style::default().fg(dim)),
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
                if key.kind != KeyEventKind::Press { continue; }

                let is_quit = matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
                if is_quit {
                    state.fade_out.store(true, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(800));
                    break;
                }

                match key.code {
                    KeyCode::Char(' ') => {
                        state.paused.store(!state.paused.load(Ordering::Relaxed), Ordering::Relaxed);
                    }
                    KeyCode::Up => { selected_row = next_row(selected_row, -1); }
                    KeyCode::Down => { selected_row = next_row(selected_row, 1); }
                    KeyCode::Right => {
                        match selected_row {
                            ROW_NOISE => {
                                let cur = state.noise_type.load(Ordering::Relaxed);
                                let next = (cur + 1) % 4;
                                state.noise_type.store(next, Ordering::Relaxed);
                                state.pending_switch.store(true, Ordering::Relaxed);
                            }
                            ROW_BIN => {
                                let cur = state.binaural_preset.load(Ordering::Relaxed);
                                let next = (cur + 1) % 6; // 0-5
                                state.binaural_preset.store(next, Ordering::Relaxed);
                                state.binaural_pending.store(true, Ordering::Relaxed);
                            }
                            ROW_RAIN => {
                                let cur = state.rain_type.load(Ordering::Relaxed);
                                let next = (cur + 1) % 4; // 0-3
                                state.rain_type.store(next, Ordering::Relaxed);
                                state.rain_pending.store(true, Ordering::Relaxed);
                            }
                            ROW_NOISE_VOL => { let mut v = state.target_noise_vol.lock().unwrap(); *v = (*v + 0.01).min(1.0); }
                            ROW_BIN_VOL => { let mut v = state.binaural_vol.lock().unwrap(); *v = (*v + 0.01).min(1.0); }
                            ROW_RAIN_VOL => { let mut v = state.rain_vol.lock().unwrap(); *v = (*v + 0.01).min(1.0); }
                            ROW_BIN_CARRIER => {
                                let mut b = state.binaural_base.lock().unwrap();
                                let step = if *b < 80.0 { 2.0 } else { 10.0 };
                                *b = (*b + step).min(400.0);
                            }
                            ROW_MOD => { let mut m = state.modulation_depth.lock().unwrap(); *m = (*m + 0.02).min(0.20); }
                            _ => {}
                        }
                    }
                    KeyCode::Left => {
                        match selected_row {
                            ROW_NOISE => {
                                let cur = state.noise_type.load(Ordering::Relaxed);
                                let next = if cur == 0 { 3 } else { cur - 1 };
                                state.noise_type.store(next, Ordering::Relaxed);
                                state.pending_switch.store(true, Ordering::Relaxed);
                            }
                            ROW_BIN => {
                                let cur = state.binaural_preset.load(Ordering::Relaxed);
                                let next = if cur == 0 { 5 } else { cur - 1 };
                                state.binaural_preset.store(next, Ordering::Relaxed);
                                state.binaural_pending.store(true, Ordering::Relaxed);
                            }
                            ROW_RAIN => {
                                let cur = state.rain_type.load(Ordering::Relaxed);
                                let next = if cur == 0 { 3 } else { cur - 1 };
                                state.rain_type.store(next, Ordering::Relaxed);
                                state.rain_pending.store(true, Ordering::Relaxed);
                            }
                            ROW_NOISE_VOL => { let mut v = state.target_noise_vol.lock().unwrap(); *v = (*v - 0.01).max(0.0); }
                            ROW_BIN_VOL => { let mut v = state.binaural_vol.lock().unwrap(); *v = (*v - 0.01).max(0.0); }
                            ROW_RAIN_VOL => { let mut v = state.rain_vol.lock().unwrap(); *v = (*v - 0.01).max(0.0); }
                            ROW_BIN_CARRIER => {
                                let mut b = state.binaural_base.lock().unwrap();
                                let step = if *b <= 80.0 { 2.0 } else { 10.0 };
                                *b = (*b - step).max(40.0);
                            }
                            ROW_MOD => { let mut m = state.modulation_depth.lock().unwrap(); *m = (*m - 0.02).max(0.0); }
                            _ => {}
                        }
                    }
                    // Quick toggles — cycle through options
                    KeyCode::Char('n') => {
                        let cur = state.noise_type.load(Ordering::Relaxed);
                        let next = (cur + 1) % 4;
                        state.noise_type.store(next, Ordering::Relaxed);
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('b') => {
                        let cur = state.binaural_preset.load(Ordering::Relaxed);
                        let next = (cur + 1) % 6;
                        state.binaural_preset.store(next, Ordering::Relaxed);
                        state.binaural_pending.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('r') => {
                        let cur = state.rain_type.load(Ordering::Relaxed);
                        let next = (cur + 1) % 4;
                        state.rain_type.store(next, Ordering::Relaxed);
                        state.rain_pending.store(true, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}
