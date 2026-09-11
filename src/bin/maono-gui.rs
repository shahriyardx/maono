//! Minimal desktop window for the Maono PD100W.
//!
//! Built on egui so the result is a single self-contained binary: no GTK or Qt
//! runtime to install, and it works the same on X11 and Wayland. The point is
//! that it runs on any Linux box the receiver is plugged into.

use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use maono::mic::{
    level_fraction, Mic, BATTERY, GAIN, GAIN_MAX, LEVEL, LIGHT, LIGHT_MODE, LIGHT_MODE_MAX, MUTE,
    NR, NR_LEVEL,
};
use std::time::{Duration, Instant};

const SEGMENTS: [&str; 4] = ["off", "low", "mid", "high"];

const BG: Color32 = Color32::from_rgb(26, 27, 38);
const PANEL: Color32 = Color32::from_rgb(36, 40, 59);
const TEXT: Color32 = Color32::from_rgb(192, 202, 245);
const FAINT: Color32 = Color32::from_rgb(110, 118, 160);
const GREEN: Color32 = Color32::from_rgb(158, 206, 106);
const AMBER: Color32 = Color32::from_rgb(224, 175, 104);
const RED: Color32 = Color32::from_rgb(247, 118, 142);
const VIOLET: Color32 = Color32::from_rgb(187, 154, 247);
const CYAN: Color32 = Color32::from_rgb(125, 207, 255);

fn zone(f: f32) -> Color32 {
    if f >= 0.85 {
        RED
    } else if f >= 0.60 {
        AMBER
    } else {
        GREEN
    }
}

struct App {
    mic: Option<Mic>,
    error: Option<String>,
    battery: u16,
    muted: bool,
    gain: u16,
    nr: usize,
    light_on: bool,
    light_mode: u16,
    level: f32,
    peak: f32,
    peak_at: Instant,
    last_battery: Instant,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            mic: None,
            error: None,
            battery: 0,
            muted: false,
            gain: 0,
            nr: 0,
            light_on: false,
            light_mode: 0,
            level: 0.0,
            peak: 0.0,
            peak_at: Instant::now(),
            last_battery: Instant::now(),
        };
        app.connect();
        app
    }

    fn connect(&mut self) {
        match Mic::open() {
            Ok(mut m) => {
                self.battery = m.get(BATTERY).ok().flatten().unwrap_or(0);
                self.muted = m.get(MUTE).ok().flatten().unwrap_or(0) != 0;
                self.gain = m.get(GAIN).ok().flatten().unwrap_or(0);
                let on = m.get(NR).ok().flatten().unwrap_or(0) != 0;
                let lvl = m.get(NR_LEVEL).ok().flatten().unwrap_or(0) as usize;
                self.nr = if on { (lvl + 1).min(3) } else { 0 };
                self.light_on = m.get(LIGHT).ok().flatten().unwrap_or(0) != 0;
                self.light_mode = m.get(LIGHT_MODE).ok().flatten().unwrap_or(0);
                self.mic = Some(m);
                self.error = None;
            }
            Err(e) => {
                self.mic = None;
                self.error = Some(e.to_string());
            }
        }
    }

    /// Take in what the device has announced, so the window follows the
    /// physical buttons as well as its own controls.
    fn poll(&mut self) {
        let Some(mic) = self.mic.as_mut() else { return };
        for (id, val) in mic.drain() {
            match id {
                LEVEL => {
                    self.level = level_fraction(val) as f32;
                    if self.level >= self.peak {
                        self.peak = self.level;
                        self.peak_at = Instant::now();
                    }
                }
                MUTE => self.muted = val != 0,
                GAIN => self.gain = val,
                NR => self.nr = if val != 0 { self.nr.max(1) } else { 0 },
                NR_LEVEL => {
                    if self.nr > 0 {
                        self.nr = (val as usize + 1).min(3)
                    }
                }
                LIGHT => self.light_on = val != 0,
                LIGHT_MODE => self.light_mode = val,
                BATTERY => self.battery = val,
                _ => {}
            }
        }
        if self.peak_at.elapsed() > Duration::from_millis(1200) {
            self.peak = (self.peak - 0.012).max(self.level);
        }
        if self.last_battery.elapsed() > Duration::from_secs(30) {
            if let Some(b) = mic.get(BATTERY).ok().flatten() {
                self.battery = b;
            }
            self.last_battery = Instant::now();
        }
    }

    fn set(&mut self, id: u16, val: u16) {
        if let Some(m) = self.mic.as_mut() {
            let _ = m.set(id, val);
        }
    }

    fn set_nr(&mut self, index: usize) {
        self.set(NR, (index > 0) as u16);
        std::thread::sleep(Duration::from_millis(90));
        self.set(NR_LEVEL, index.saturating_sub(1) as u16);
        self.nr = index;
    }

    fn step_light(&mut self, delta: i32) {
        let span = LIGHT_MODE_MAX as i32 + 1;
        let next = ((self.light_mode as i32 + delta) % span + span) % span;
        self.set(LIGHT, 1);
        std::thread::sleep(Duration::from_millis(90));
        self.set(LIGHT_MODE, next as u16);
        self.light_on = true;
        self.light_mode = next as u16;
    }
}

/// Horizontal level meter, drawn directly so it can carry a peak marker.
fn meter(ui: &mut egui::Ui, level: f32, peak: f32, muted: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(3), PANEL);
    if !muted && level > 0.0 {
        let mut fill = rect;
        fill.set_width(rect.width() * level.clamp(0.0, 1.0));
        p.rect_filled(fill, CornerRadius::same(3), zone(level));
    }
    if !muted && peak > 0.02 {
        let x = rect.left() + rect.width() * peak.clamp(0.0, 1.0);
        p.vline(x, rect.y_range(), Stroke::new(2.0, TEXT));
    }
}

fn pill(ui: &mut egui::Ui, text: &str, selected: bool, colour: Color32) -> egui::Response {
    let (bg, fg) = if selected { (colour, BG) } else { (PANEL, FAINT) };
    ui.add(
        egui::Button::new(RichText::new(text).color(fg).size(13.0))
            .fill(bg)
            .corner_radius(CornerRadius::same(4))
            .min_size(Vec2::new(52.0, 26.0)),
    )
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        // the level meter animates, so keep asking for frames
        ui.ctx().request_repaint_after(Duration::from_millis(50));

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.override_text_color = Some(TEXT);
        ui.ctx().set_visuals(visuals);

        {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

            if let Some(err) = self.error.clone() {
                ui.add_space(12.0);
                ui.heading(RichText::new("microphone not available").color(RED));
                ui.label(RichText::new(err).color(FAINT).size(12.0));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Plug in the PD100W receiver. If it is plugged in, the udev \
                         rule is missing - see 99-maono.rules in the project.",
                    )
                    .color(FAINT)
                    .size(12.0),
                );
                if ui.button("retry").clicked() {
                    self.connect();
                }
                return;
            }

            // ---- header --------------------------------------------------
            ui.horizontal(|ui| {
                ui.label(RichText::new("maono pd100w").color(TEXT).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let bc = match self.battery {
                        0..=15 => RED,
                        16..=35 => AMBER,
                        _ => GREEN,
                    };
                    ui.label(RichText::new(format!("{}%", self.battery)).color(bc));
                    ui.label(RichText::new("battery").color(FAINT).size(12.0));
                });
            });
            ui.separator();

            // ---- level ---------------------------------------------------
            ui.horizontal(|ui| {
                ui.label(RichText::new("level").color(FAINT).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(if self.muted {
                            "muted".into()
                        } else {
                            format!("{:>3.0}%", self.level * 100.0)
                        })
                        .color(if self.muted { FAINT } else { zone(self.level) })
                        .size(12.0),
                    );
                });
            });
            meter(ui, self.level, self.peak, self.muted);

            // ---- mute ----------------------------------------------------
            ui.add_space(4.0);
            let (fill, text) = if self.muted {
                (RED, "MUTED  —  click to go live")
            } else {
                (GREEN, "LIVE  —  click to mute")
            };
            if ui
                .add_sized(
                    Vec2::new(ui.available_width(), 38.0),
                    egui::Button::new(RichText::new(text).color(BG).strong().size(14.0))
                        .fill(fill)
                        .corner_radius(CornerRadius::same(5)),
                )
                .clicked()
            {
                let want = !self.muted;
                self.set(MUTE, want as u16);
                self.muted = want;
            }

            // ---- gain ----------------------------------------------------
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("gain").color(FAINT).size(12.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} / {GAIN_MAX}", self.gain))
                            .color(TEXT)
                            .size(12.0),
                    );
                });
            });
            let mut g = self.gain as f32;
            if ui
                .add(
                    egui::Slider::new(&mut g, 0.0..=GAIN_MAX as f32)
                        .show_value(false)
                        .trailing_fill(true),
                )
                .changed()
            {
                let want = g.round() as u16;
                self.set(GAIN, want);
                self.gain = want;
            }

            // ---- noise reduction -----------------------------------------
            ui.add_space(4.0);
            ui.label(RichText::new("noise reduction").color(FAINT).size(12.0));
            ui.horizontal(|ui| {
                for (i, name) in SEGMENTS.iter().enumerate() {
                    if pill(ui, name, i == self.nr, VIOLET).clicked() {
                        self.set_nr(i);
                    }
                }
            });

            // ---- rgb light -----------------------------------------------
            ui.add_space(4.0);
            ui.label(RichText::new("rgb light").color(FAINT).size(12.0));
            ui.horizontal(|ui| {
                if pill(
                    ui,
                    if self.light_on { "on" } else { "off" },
                    self.light_on,
                    AMBER,
                )
                .clicked()
                {
                    let want = !self.light_on;
                    self.set(LIGHT, want as u16);
                    self.light_on = want;
                }
                if pill(ui, "◀", false, CYAN).clicked() {
                    self.step_light(-1);
                }
                ui.label(
                    RichText::new(format!("mode {}", self.light_mode))
                        .color(if self.light_on { AMBER } else { FAINT })
                        .size(12.0),
                );
                if pill(ui, "▶", false, CYAN).clicked() {
                    self.step_light(1);
                }
            });
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 430.0])
            .with_min_inner_size([300.0, 380.0])
            .with_title("Maono PD100W"),
        ..Default::default()
    };
    eframe::run_native(
        "maono-gui",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}
