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
    let mut last_noise: Option<u8> = None;
    let mut last_binaural: Option<u8> = None;
    let mut last_rain: Option<u8> = None;
    let mut show_help = false;
    let mut compact = false;
    let mut timer_end = timer_end;
    let mut timer_mode: u8 = if timer_end.is_some() { 1 } else { 0 }; // 0=off, 1=45m, 2=1h
    let mut timer_fired = false;
    let mut signal_at: Option<Instant> = None;

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
            if Instant::now() >= end && !timer_fired {
                *state.fade_out_duration.lock().unwrap() = 5.0;
                state.fade_out.store(true, Ordering::Relaxed);
                signal_at = Some(Instant::now() + Duration::from_secs(2));
                timer_fired = true;
            }
        }

        if let Some(at) = signal_at {
            if Instant::now() >= at {
                state.timer_signal.store(true, Ordering::Relaxed);
                signal_at = None;
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
            if !compact {
                lines.push(Line::from(Span::styled("  ──────────────────────────────────────────", Style::default().fg(subtle))));
            }

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

            // Helper: source label — first letter always colored (keybind hint), rest colored when active
            let source_label = |name: &str, row: usize, color: Color, active: bool| -> Vec<Span<'static>> {
                let first = &name[..1];
                let rest = &name[1..];
                let rest_c = if active { color } else { surface };
                if selected_row == row {
                    vec![
                        Span::styled(" [".to_string(), Style::default().fg(color)),
                        Span::styled(first.to_string(), Style::default().fg(color)),
                        Span::styled(rest.to_string(), Style::default().fg(rest_c)),
                        Span::styled("] ".to_string(), Style::default().fg(color)),
                    ]
                } else {
                    vec![
                        Span::styled(format!("  {first}"), Style::default().fg(color)),
                        Span::styled(format!("{rest}  "), Style::default().fg(rest_c)),
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
            let mut noise_line: Vec<Span> = source_label("noise", ROW_NOISE, c_noise, noise_active);
            if compact {
                let name = NOISE_NAMES[noise_idx];
                noise_line.push(Span::styled(name.to_string(), Style::default().fg(text)));
            } else {
                noise_line.extend(source_row(&NOISE_NAMES, noise_idx, noise_active));
            }
            lines.push(Line::from(noise_line));

            // Binaural source row
            let mut bin_line: Vec<Span> = source_label("bin", ROW_BIN, c_bin, bin_active);
            if compact {
                let name = BIN_NAMES[bin_preset];
                bin_line.push(Span::styled(format!("  {name}"), Style::default().fg(text)));
            } else {
                bin_line.push(Span::raw("  "));
                bin_line.extend(source_row(&BIN_NAMES, bin_preset, bin_active));
            }
            lines.push(Line::from(bin_line));

            // Rain source row
            let mut rain_line: Vec<Span> = source_label("rain", ROW_RAIN, c_rain, rain_active);
            if compact {
                let name = RAIN_NAMES[rain_type];
                rain_line.push(Span::styled(format!(" {name}"), Style::default().fg(text)));
            } else {
                rain_line.push(Span::raw(" "));
                rain_line.extend(source_row(&RAIN_NAMES, rain_type, rain_active));
            }
            lines.push(Line::from(rain_line));

            // Separator
            if !compact { lines.push(Line::from("")); }

            // Volume/control bars
            let bar_len = if compact { 12 } else { 20 };

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
            if !compact {
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
            }

            // Timer
            if let Some(ref t) = timer_str {
                let mode_label = match timer_mode { 1 => "15m", 2 => "45m", 3 => "1h", _ => "" };
                lines.push(Line::from(vec![
                    Span::styled("  tmr ", Style::default().fg(dim)),
                    Span::styled(t.clone(), Style::default().fg(yellow)),
                    Span::styled(format!("  {mode_label}"), Style::default().fg(dim)),
                ]));
            }

            // Keep animation time updated even in compact mode
            let now_inst = Instant::now();
            if !paused { anim_time += (now_inst - last_frame).as_secs_f32(); }
            last_frame = now_inst;

            // Animations — three side by side: noise slope, binaural pulse, rain drops
            if !compact {
            let elapsed = anim_time;
            let anim_rows = 3;
            let anim_w = 10;

            // --- Noise spectrum bars ---
            // Bar heights reflect frequency profile: white=flat, pink=tapers, brown=steep dropoff
            // Modulation drives jitter speed
            // --- Noise spectrum bars ---
            // Bar heights reflect frequency profile: white=flat, pink=tapers, brown=steep dropoff
            // LFO rate in audio is 0.04 Hz — match that slow drift; depth controls amplitude
            let bar_count = anim_w;
            let noise_bars: Vec<f32> = {
                let mut bars = Vec::new();
                let lfo_rate = 0.04_f32; // matches audio LFO
                for i in 0..bar_count {
                    let x = i as f32 / (bar_count - 1) as f32; // 0=low freq, 1=high freq
                    let base = match noise_idx {
                        1 => 0.7,                          // white: flat
                        2 => 0.9 - x * 0.5,               // pink: gentle slope
                        3 => 0.95 - x * x * 0.85,         // brown: steep curve
                        _ => 0.0,
                    };
                    let jitter = if noise_active {
                        let seed = i as f32 * 2.7 + 0.3;
                        let t = elapsed * lfo_rate * std::f32::consts::TAU;
                        ((t + seed).sin() * 0.6 + (t * 1.7 + seed * 3.1).sin() * 0.4) * modulation
                    } else { 0.0 };
                    bars.push((base + jitter).clamp(0.0, 1.0));
                }
                bars
            };

            // --- Binaural L/R pulse ---
            // Beat freq mapped to a comfortable visual range: delta 2Hz→slow, gamma 40Hz→fast
            // log scale so the range feels proportional
            let visual_hz = if bin_active && binaural > 0.0 {
                let min_hz = 2.0_f32;
                let max_hz = 40.0_f32;
                let min_vis = 0.15_f32;
                let max_vis = 1.5_f32;
                let t = ((binaural / min_hz).ln() / (max_hz / min_hz).ln()).clamp(0.0, 1.0);
                min_vis + t * (max_vis - min_vis)
            } else { 0.0 };
            // Focus position sweeps -1 (left) to +1 (right) and back
            let bin_focus = if bin_active {
                (elapsed * visual_hz * std::f32::consts::TAU).sin()
            } else { 0.0 };

            // --- Rain drop animation ---
            let rain_anim_w: usize = 10;
            let all_drops: &[(usize, f32, f32)] = &[
                (1, 1.1, 0.0), (4, 0.8, 0.4), (8, 1.3, 0.7),
                (2, 0.9, 0.2), (6, 1.4, 0.5), (9, 0.7, 0.9),
                (0, 1.2, 0.1), (3, 1.6, 0.3), (5, 0.6, 0.6), (7, 1.0, 0.8),
            ];
            let drop_count = match rain_type { 1 => 3, 2 => 6, 3 => 10, _ => 0 };
            let rain_light = if rain_active { Color::Rgb(100, 170, 220) } else { surface };
            let rain_dim_c = if rain_active { Color::Rgb(50, 90, 130) } else { surface };

            lines.push(Line::from(""));
            for row_idx in 0..anim_rows {
                let mut spans: Vec<Span> = vec![Span::raw("  ")];

                // Noise spectrum bars
                let bar_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
                for i in 0..bar_count {
                    let h = noise_bars[i];
                    let bar_rows = h * anim_rows as f32;
                    let row_from_bottom = (anim_rows - 1 - row_idx) as f32;
                    let fill = bar_rows - row_from_bottom;
                    let c = if !noise_active { surface } else { c_noise };
                    if fill >= 1.0 {
                        spans.push(Span::styled("█", Style::default().fg(c)));
                    } else if fill > 0.0 {
                        let idx = (fill * (bar_chars.len() - 1) as f32) as usize;
                        spans.push(Span::styled(bar_chars[idx].to_string(), Style::default().fg(c)));
                    } else {
                        spans.push(Span::styled(" ", Style::default().fg(surface)));
                    }
                }

                spans.push(Span::raw("  "));

                // Binaural L/R pulse — single spot sweeping left↔right
                let bin_w = anim_w;
                for i in 0..bin_w {
                    let col_pos = (i as f32 / (bin_w - 1) as f32) * 2.0 - 1.0; // -1..+1
                    let dist = (col_pos - bin_focus).abs();
                    let intensity = (1.0 - dist * 0.8).clamp(0.0, 1.0);

                    // L/R labels at edges
                    if i == 0 && row_idx == 1 {
                        let br = intensity;
                        let c = if !bin_active { surface }
                            else { Color::Rgb(
                                (80.0 + br * 123.0) as u8,
                                (70.0 + br * 96.0) as u8,
                                (110.0 + br * 137.0) as u8,
                            )};
                        spans.push(Span::styled("L", Style::default().fg(c)));
                        continue;
                    }
                    if i == bin_w - 1 && row_idx == 1 {
                        let br = intensity;
                        let c = if !bin_active { surface }
                            else { Color::Rgb(
                                (80.0 + br * 123.0) as u8,
                                (70.0 + br * 96.0) as u8,
                                (110.0 + br * 137.0) as u8,
                            )};
                        spans.push(Span::styled("R", Style::default().fg(c)));
                        continue;
                    }

                    let show = match row_idx {
                        1 => intensity > 0.05,
                        0 | 2 => intensity > 0.4,
                        _ => false,
                    };
                    if show && bin_active {
                        let ch = if intensity > 0.7 { '█' }
                            else if intensity > 0.4 { '▓' }
                            else { '░' };
                        let c = Color::Rgb(
                            (55.0 + intensity * 148.0) as u8,
                            (57.0 + intensity * 109.0) as u8,
                            (75.0 + intensity * 172.0) as u8,
                        );
                        spans.push(Span::styled(ch.to_string(), Style::default().fg(c)));
                    } else {
                        spans.push(Span::styled(" ", Style::default().fg(surface)));
                    }
                }

                spans.push(Span::raw("  "));

                // Rain drops
                let mut rain_chars = vec![(' ', 0u8); rain_anim_w];
                if rain_active {
                    for (i, &(col, speed, offset)) in all_drops.iter().take(drop_count).enumerate() {
                        let ps = offset + (i as f32 * 1.618);
                        let dp = (elapsed * speed + ps) % 1.0;
                        let dr = (dp * (anim_rows as f32 + 1.0)) as usize;
                        if col < rain_anim_w {
                            if dr == row_idx { rain_chars[col] = ('|', 2); }
                            else if dr == row_idx + 1 && rain_chars[col].1 < 1 { rain_chars[col] = ('·', 1); }
                        }
                    }
                }
                for &(ch, _) in &rain_chars {
                    match ch {
                        '|' => spans.push(Span::styled("|", Style::default().fg(rain_light))),
                        '·' => spans.push(Span::styled("·", Style::default().fg(rain_dim_c))),
                        _ => spans.push(Span::raw(" ")),
                    }
                }

                lines.push(Line::from(spans));
            }

            lines.push(Line::from(""));
            } // end !compact animations

            if !compact {
                lines.push(Line::from(Span::styled("  ──────────────────────────────────────────", Style::default().fg(subtle))));

                lines.push(Line::from(vec![
                    Span::styled("  i", Style::default().fg(text)),
                    Span::styled("nfo  ", Style::default().fg(dim)),
                    Span::styled("t", Style::default().fg(yellow)),
                    Span::styled("imer  ", Style::default().fg(dim)),
                    Span::styled("m", Style::default().fg(green)),
                    Span::styled("ute  ", Style::default().fg(dim)),
                    Span::styled("c", Style::default().fg(text)),
                    Span::styled("ompact  ", Style::default().fg(dim)),
                    Span::styled("q", Style::default().fg(red)),
                    Span::styled("uit", Style::default().fg(dim)),
                ]));
            }

            let block = if compact {
                Block::default()
                    .padding(Padding::new(1, 0, 0, 0))
            } else {
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(surface))
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0))
            };

            let paragraph = Paragraph::new(lines).block(block);
            frame.render_widget(paragraph, area);

            if show_help {
                let help_lines = vec![
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  navigation", Style::default().fg(text).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("  hjkl / arrows   ", Style::default().fg(text)),
                        Span::styled("move & adjust", Style::default().fg(dim)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  sources", Style::default().fg(text).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("  n", Style::default().fg(c_noise)),
                        Span::styled(" / ", Style::default().fg(subtle)),
                        Span::styled("b", Style::default().fg(c_bin)),
                        Span::styled(" / ", Style::default().fg(subtle)),
                        Span::styled("r", Style::default().fg(c_rain)),
                        Span::styled("               cycle source type", Style::default().fg(dim)),
                    ]),
                    Line::from(vec![
                        Span::styled("  N", Style::default().fg(c_noise)),
                        Span::styled(" / ", Style::default().fg(subtle)),
                        Span::styled("B", Style::default().fg(c_bin)),
                        Span::styled(" / ", Style::default().fg(subtle)),
                        Span::styled("R", Style::default().fg(c_rain)),
                        Span::styled("               toggle on/off", Style::default().fg(dim)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  playback", Style::default().fg(text).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::styled("  t                 ", Style::default().fg(yellow)),
                        Span::styled("timer: 15m → 45m → 1h → off", Style::default().fg(dim)),
                    ]),
                    Line::from(vec![
                        Span::styled("  m                 ", Style::default().fg(green)),
                        Span::styled("mute/unmute", Style::default().fg(dim)),
                    ]),
                    Line::from(vec![
                        Span::styled("  c                 ", Style::default().fg(text)),
                        Span::styled("compact mode", Style::default().fg(dim)),
                    ]),
                    Line::from(vec![
                        Span::styled("  q / esc           ", Style::default().fg(red)),
                        Span::styled("quit", Style::default().fg(dim)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  press any key to close", Style::default().fg(subtle)),
                    ]),
                ];

                let help_height = help_lines.len() as u16 + 2;
                let help_width = 40u16;
                let help_x = area.x + (area.width.saturating_sub(help_width)) / 2;
                let help_y = area.y + (area.height.saturating_sub(help_height)) / 2;
                let help_area = ratatui::layout::Rect::new(help_x, help_y, help_width, help_height);

                let help_block = Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(dim))
                    .border_type(ratatui::widgets::BorderType::Rounded);
                let help_paragraph = Paragraph::new(help_lines).block(help_block);

                frame.render_widget(ratatui::widgets::Clear, help_area);
                frame.render_widget(help_paragraph, help_area);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press { continue; }

                let is_quit = matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL));
                if is_quit {
                    if show_help {
                        show_help = false;
                        continue;
                    }
                    state.fade_out.store(true, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(800));
                    break;
                }

                if show_help {
                    show_help = false;
                    continue;
                }

                match key.code {
                    KeyCode::Char('c') => { compact = !compact; }
                    KeyCode::Char('t') => {
                        timer_mode = (timer_mode + 1) % 4;
                        timer_end = match timer_mode {
                            1 => Some(Instant::now() + Duration::from_secs(15 * 60)),
                            2 => Some(Instant::now() + Duration::from_secs(45 * 60)),
                            3 => Some(Instant::now() + Duration::from_secs(60 * 60)),
                            _ => None,
                        };
                        state.fade_out.store(false, Ordering::Relaxed);
                        state.paused.store(false, Ordering::Relaxed);
                        timer_fired = false;
                        signal_at = None;
                        if timer_end.is_some() {
                            state.tone_click.store(true, Ordering::Relaxed);
                        }
                    }
                    KeyCode::Char('i') => { show_help = true; }
                    KeyCode::Char('m') => {
                        let was_paused = state.paused.load(Ordering::Relaxed);
                        if was_paused && state.fade_out.load(Ordering::Relaxed) {
                            state.fade_out.store(false, Ordering::Relaxed);
                        }
                        state.paused.store(!was_paused, Ordering::Relaxed);
                    }
                    KeyCode::Up | KeyCode::Char('k') => { selected_row = next_row(selected_row, -1); }
                    KeyCode::Down | KeyCode::Char('j') => { selected_row = next_row(selected_row, 1); }
                    KeyCode::Right | KeyCode::Char('l') => {
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
                    KeyCode::Left | KeyCode::Char('h') => {
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
                    // Shift toggles — on/off for each source (remembers last active setting)
                    KeyCode::Char('N') => {
                        let cur = state.noise_type.load(Ordering::Relaxed);
                        if cur == 0 {
                            state.noise_type.store(last_noise.unwrap_or(3), Ordering::Relaxed);
                        } else {
                            last_noise = Some(cur);
                            state.noise_type.store(0, Ordering::Relaxed);
                        }
                        state.pending_switch.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('B') => {
                        let cur = state.binaural_preset.load(Ordering::Relaxed);
                        if cur == 0 {
                            state.binaural_preset.store(last_binaural.unwrap_or(1), Ordering::Relaxed);
                        } else {
                            last_binaural = Some(cur);
                            state.binaural_preset.store(0, Ordering::Relaxed);
                        }
                        state.binaural_pending.store(true, Ordering::Relaxed);
                    }
                    KeyCode::Char('R') => {
                        let cur = state.rain_type.load(Ordering::Relaxed);
                        if cur == 0 {
                            state.rain_type.store(last_rain.unwrap_or(1), Ordering::Relaxed);
                        } else {
                            last_rain = Some(cur);
                            state.rain_type.store(0, Ordering::Relaxed);
                        }
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
