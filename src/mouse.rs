//! Mouse / touchpad test: buttons, scroll directions, double-click, click chatter and a drawing area.

use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};

/// Two presses of the same button closer than this are a faulty switch, not a person.
/// Nobody clicking slowly gets near it, so there are no false alarms.
const CLICK_CHATTER_SECS: f64 = 0.05;
/// Slow clicks the double-click test asks for.
const DC_TARGET: u32 = 20;
/// How many recent gaps between clicks to show per button.
const GAPS_SHOWN: usize = 6;
/// Presses of the same button closer than this form a streak (double, triple…). Windows' default double-click time.
const STREAK_SECS: f64 = 0.5;
/// How long the pointer trail stays visible.
const TRAIL_SECS: f64 = 2.5;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Scroll {
    Up,
    Down,
    Left,
    Right,
}

pub const BUTTONS: [(PointerButton, &str); 5] = [
    (PointerButton::Primary, "Left"),
    (PointerButton::Middle, "Middle"),
    (PointerButton::Secondary, "Right"),
    (PointerButton::Extra1, "Back"),
    (PointerButton::Extra2, "Forward"),
];

/// Index of a button in `BUTTONS` (egui's PointerButton can't be a map key).
fn idx(b: PointerButton) -> Option<usize> {
    BUTTONS.iter().position(|(x, _)| *x == b)
}

#[derive(Default)]
pub struct MouseTest {
    down: [bool; 5],
    /// Which of `BUTTONS` have been pressed.
    pub tested: [bool; 5],
    pub scrolled: HashSet<Scroll>,
    scroll_at: HashMap<Scroll, f64>,
    last_press: [Option<f64>; 5],
    /// Click chatter per button in `BUTTONS`: how many times one click registered as two.
    pub chatter: [u32; 5],
    /// Recent gaps (seconds) between presses of each button, newest last.
    gaps: [VecDeque<f64>; 5],
    /// Shortest gap ever seen per button.
    min_gap: [Option<f64>; 5],
    /// Real clicks per button (chatter not included).
    pub clicks: [u32; 5],
    /// Current run of quick clicks per button, and when its last click happened.
    streak: [u32; 5],
    streak_at: [f64; 5],
    /// Button of the most recent click, for the pop-up in the drawing area.
    last_button: Option<usize>,
    pub double_clicked: bool,
    /// Double-click issue test: (clicks, chatter) per button when it started.
    dc_test: Option<([u32; 5], [u32; 5])>,
    /// When the last unwanted double-click happened (for the flash).
    chatter_at: f64,
    dev_dc: bool,
    trail: VecDeque<(Pos2, f64)>,
    /// Height the page may fill (set by the app each frame).
    pub fit_height: f32,
}


impl MouseTest {
    fn handle_input(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        for ev in ctx.input(|i| i.events.clone()) {
            match ev {
                egui::Event::PointerButton { button, pressed: true, .. } => {
                    let Some(i) = idx(button) else { continue };
                    self.down[i] = true;
                    self.tested[i] = true;
                    self.press(i, now);
                }
                egui::Event::PointerButton { button, pressed: false, .. } => {
                    if let Some(i) = idx(button) {
                        self.down[i] = false;
                    }
                }
                egui::Event::MouseWheel { delta, .. } => {
                    let dirs = [
                        (delta.y > 0.0, Scroll::Up),
                        (delta.y < 0.0, Scroll::Down),
                        (delta.x > 0.0, Scroll::Left),
                        (delta.x < 0.0, Scroll::Right),
                    ];
                    for (hit, d) in dirs {
                        if hit {
                            self.scrolled.insert(d);
                            self.scroll_at.insert(d, now);
                        }
                    }
                }
                egui::Event::WindowFocused(false) | egui::Event::PointerGone => self.down = [false; 5],
                _ => {}
            }
        }
    }

    /// Count one press of `BUTTONS[i]`. Only the same button pressed again quickly
    /// continues a streak, so Right then Left is never a double-click.
    fn press(&mut self, i: usize, now: f64) {
        if let Some(prev) = self.last_press[i] {
            let gap = now - prev;
            self.gaps[i].push_back(gap);
            if self.gaps[i].len() > GAPS_SHOWN {
                self.gaps[i].pop_front();
            }
            self.min_gap[i] = Some(self.min_gap[i].map_or(gap, |m| m.min(gap)));
        }
        if self.last_press[i].replace(now).is_some_and(|prev| now - prev < CLICK_CHATTER_SECS) {
            self.chatter[i] += 1;
            self.chatter_at = now;
            return;
        }
        self.clicks[i] += 1;
        let continues = self.last_button == Some(i) && now - self.streak_at[i] < STREAK_SECS;
        self.streak[i] = if continues { self.streak[i] + 1 } else { 1 };
        self.streak_at[i] = now;
        self.last_button = Some(i);
        if i == 0 && self.streak[i] >= 2 {
            self.double_clicked = true;
        }
    }

    /// Double-click test results per button since it started: (clicks, unwanted doubles).
    fn dc_counts(&self) -> Option<[(u32, u32); 5]> {
        let (c0, h0) = self.dc_test?;
        Some(std::array::from_fn(|i| (self.clicks[i] - c0[i], self.chatter[i] - h0[i])))
    }

    fn start_dc_test(&mut self) {
        self.dc_test = Some((self.clicks, self.chatter));
        self.last_button = None;
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        let n = self.tested.iter().filter(|t| **t).count();
        let chatter: Vec<String> = (0..5).filter(|i| self.chatter[*i] > 0).map(|i| format!("{} ×{}", BUTTONS[i].1, self.chatter[i])).collect();
        let basics = self.tested[0] && self.tested[1] && self.tested[2]
            && self.scrolled.contains(&Scroll::Up) && self.scrolled.contains(&Scroll::Down);
        let status = if !chatter.is_empty() {
            Status::Issue
        } else if n == 0 && self.scrolled.is_empty() {
            Status::NotTested
        } else if basics {
            Status::Pass
        } else {
            Status::InProgress
        };
        let names = |want: bool| -> String {
            let v: Vec<&str> = BUTTONS.iter().enumerate().filter(|(i, _)| self.tested[*i] == want).map(|(_, (_, n))| *n).collect();
            if v.is_empty() { "none".into() } else { v.join(", ") }
        };
        let mut lines = vec![format!("Buttons working: {}", names(true))];
        if n < 5 {
            lines.push(format!("Not clicked yet: {}", names(false)));
        }
        let dirs: Vec<&str> = [(Scroll::Up, "up"), (Scroll::Down, "down"), (Scroll::Left, "left"), (Scroll::Right, "right")]
            .iter()
            .filter(|(d, _)| self.scrolled.contains(d))
            .map(|(_, n)| *n)
            .collect();
        lines.push(format!("Scroll: {}", if dirs.is_empty() { "not tested".into() } else { dirs.join(", ") }));
        let counts: Vec<String> = (0..5).filter(|i| self.clicks[*i] > 0).map(|i| format!("{} {}", BUTTONS[i].1, self.clicks[i])).collect();
        if !counts.is_empty() {
            lines.push(format!("Clicks: {}", counts.join(", ")));
        }
        let shortest: Vec<String> = (0..5).filter_map(|i| self.min_gap[i].map(|g| format!("{} {}", BUTTONS[i].1, fmt_gap(g)))).collect();
        if !shortest.is_empty() {
            lines.push(format!("Shortest gap between clicks: {} (under 50 ms = faulty switch)", shortest.join(", ")));
        }
        lines.push(format!("Double-click: {}", if self.double_clicked { "works" } else { "not tested" }));
        if let Some(dc) = self.dc_counts() {
            let parts: Vec<String> = (0..5)
                .filter(|i| dc[*i].0 + dc[*i].1 > 0)
                .map(|i| format!("{} {} clicks, {} unwanted", BUTTONS[i].1, dc[i].0, dc[i].1))
                .collect();
            if !parts.is_empty() {
                lines.push(format!("Double-click test: {}", parts.join("; ")));
            }
        }
        if !chatter.is_empty() {
            lines.push(format!("Click chatter (one click counted twice): {}", chatter.join(", ")));
        }
        Section { title: "Mouse", jp: "マウス", status, lines }
    }

    pub fn reset(&mut self) {
        *self = Self { fit_height: self.fit_height, ..Self::default() };
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        self.handle_input(ui.ctx());
        let now = ui.input(|i| i.time);
        let n_tested = self.tested.iter().filter(|t| **t).count();
        let worst = (0..5).filter(|i| self.chatter[*i] > 0).max_by_key(|i| self.chatter[*i]);

        if !self.dev_dc {
            self.dev_dc = true;
            if std::env::var("NYANSPECT_DC_TEST").is_ok() {
                self.start_dc_test();
            }
        }
        let dc = self.dc_counts();
        if let Some(dc) = dc {
            if dc.iter().any(|(_, bad)| *bad > 0) {
                mascot.say("Caught an unwanted double-click! (；ω；)");
            } else if dc.iter().any(|(n, _)| *n >= DC_TARGET) {
                mascot.say("No sneaky double-clicks~ that switch is healthy ♪");
            } else {
                mascot.say("Click slowly~ I'm watching for sneaky double-clicks");
            }
        } else if let Some(i) = worst {
            mascot.say(format!("{} clicked twice by itself… worn switch? (；ω；)", BUTTONS[i].1));
        } else if n_tested == 0 {
            mascot.say("Click every mouse button and scroll~ ♪");
        } else if n_tested >= 3 && self.scrolled.len() >= 2 {
            mascot.say("Your mouse looks healthy! (≧▽≦)");
        } else {
            mascot.say("Try the side buttons and scroll too~");
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Mouse & touchpad", "マウス", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::fancy_button(ui, "↺ Reset", Vec2::new(100.0, 32.0), theme::SILVER, theme::STEEL, true).clicked() {
                        self.reset();
                    }
                    ui.add_space(6.0);
                    let (label, c1) = if self.dc_test.is_some() { ("■ Stop test", theme::LIGHT) } else { ("Double-click test", theme::ACCENT) };
                    if theme::fancy_button(ui, label, Vec2::new(170.0, 32.0), c1, theme::SILVER, true)
                        .on_hover_text("Click slowly 20 times; any unwanted double-click is caught")
                        .clicked()
                    {
                        if self.dc_test.is_some() {
                            self.dc_test = None;
                        } else {
                            self.start_dc_test();
                        }
                    }
                });
            });
            ui.label(
                egui::RichText::new("Click each button slowly, about once a second: a healthy switch never shows a gap under 50 ms. Then scroll every way, double-click and draw.")
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);

            // Fill the window: the card's other rows need about 190 px (+30 with the chatter line).
            let gap_rows = self.gaps.iter().filter(|g| !g.is_empty()).count() as f32;
            let reserved = 190.0 + gap_rows * 40.0 + if self.chatter.iter().any(|c| *c > 0) { 32.0 } else { 0.0 };
            let h = (self.fit_height - reserved).clamp(300.0, 700.0);
            ui.horizontal_top(|ui| {
                // Mouse diagram
                let (area, _) = ui.allocate_exact_size(Vec2::new(230.0, h), Sense::hover());
                self.draw_mouse(ui, area, now);

                ui.add_space(16.0);
                // Drawing / double-click area
                let (pad, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click_and_drag());
                if let Some(pos) = resp.hover_pos() {
                    if self.trail.back().is_none_or(|(p, _)| p.distance(pos) > 0.5) {
                        self.trail.push_back((pos, now));
                    }
                }
                while self.trail.front().is_some_and(|(_, t)| now - t > TRAIL_SECS) {
                    self.trail.pop_front();
                }
                let p = ui.painter_at(pad);
                p.rect_filled(pad, 16.0, Color32::from_rgba_unmultiplied(6, 6, 8, 210));
                p.rect_stroke(pad, 16.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.15)), StrokeKind::Inside);
                // dotted grid
                let mut y = pad.top() + 20.0;
                while y < pad.bottom() {
                    let mut x = pad.left() + 20.0;
                    while x < pad.right() {
                        p.circle_filled(Pos2::new(x, y), 1.0, alpha(Color32::WHITE, 0.10));
                        x += 24.0;
                    }
                    y += 24.0;
                }
                if let Some(dc) = dc {
                    self.draw_dc_test(&p, pad, &dc, now);
                } else {
                for w in self.trail.iter().collect::<Vec<_>>().windows(2) {
                    let a = (1.0 - (now - w[1].1) / TRAIL_SECS) as f32;
                    p.line_segment([w[0].0, w[1].0], Stroke::new(1.0 + 3.0 * a, alpha(Color32::WHITE, a)));
                }
                if self.trail.is_empty() {
                    p.text(pad.center(), Align2::CENTER_CENTER, "Move, click and double-click here", theme::bold(16.0), alpha(theme::TEXT_DIM, 0.8));
                }
                // Click streak pop-up: "Left ×1", "Left ×2 Double!", "Left ×3 Triple!"…
                if let Some(i) = self.last_button {
                    let since = now - self.streak_at[i];
                    if since < 1.2 {
                        let a = (1.0 - since / 1.2) as f32;
                        let n = self.streak[i];
                        let word = match n {
                            1 => "",
                            2 => "  Double!",
                            3 => "  Triple!",
                            _ => "  Combo!",
                        };
                        let bump = (1.0 - since / 0.15).clamp(0.0, 1.0) as f32;
                        p.text(
                            pad.center_top() + Vec2::new(0.0, 34.0),
                            Align2::CENTER_CENTER,
                            format!("{} ×{n}{word}", BUTTONS[i].1),
                            theme::bold(22.0 + 6.0 * bump),
                            alpha(Color32::WHITE, a),
                        );
                    }
                }
                }
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                for (i, (_, name)) in BUTTONS.iter().enumerate() {
                    let label = if self.clicks[i] > 0 { format!("{name} {}", self.clicks[i]) } else { name.to_string() };
                    chip(ui, &label, self.tested[i]);
                }
                ui.add_space(8.0);
                for (d, name) in [(Scroll::Up, "Scroll ↑"), (Scroll::Down, "Scroll ↓"), (Scroll::Left, "Scroll ←"), (Scroll::Right, "Scroll →")] {
                    chip(ui, name, self.scrolled.contains(&d));
                }
                ui.add_space(8.0);
                chip(ui, "Double-click", self.double_clicked);
            });
            // Gap readout: one row per button that has been clicked twice or more.
            for i in 0..5 {
                if self.gaps[i].is_empty() {
                    continue;
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add_sized(
                        Vec2::new(70.0, 20.0),
                        egui::Label::new(egui::RichText::new(BUTTONS[i].1).font(theme::bold(13.0)).color(theme::TEXT)),
                    );
                    ui.label(egui::RichText::new("gaps").size(12.0).color(theme::TEXT_DIM));
                    for &g in &self.gaps[i] {
                        let bad = g < CLICK_CHATTER_SECS;
                        egui::Frame::new()
                            .fill(if bad { theme::GLINT } else { alpha(theme::SILVER, 0.12) })
                            .corner_radius(8)
                            .inner_margin(egui::Margin::symmetric(7, 1))
                            .show(ui, |ui| {
                                let t = if bad { format!("⚠ {}", fmt_gap(g)) } else { fmt_gap(g) };
                                ui.label(egui::RichText::new(t).size(12.5).color(if bad { theme::INK } else { theme::TEXT }));
                            });
                    }
                });
            }
            if worst.is_some() {
                ui.add_space(4.0);
                let list: Vec<String> = (0..5)
                    .filter(|i| self.chatter[*i] > 0)
                    .map(|i| format!("{} ×{}", BUTTONS[i].1, self.chatter[i]))
                    .collect();
                ui.label(
                    egui::RichText::new(format!("⚠ Click chatter (one click counted twice): {}", list.join(", ")))
                        .font(theme::bold(13.0))
                        .color(Color32::WHITE),
                );
            }
            ui.label(
                egui::RichText::new("Sideways scroll needs a tilt wheel or a two-finger swipe on a touchpad. Back/Forward are the side buttons, if your mouse has them.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
        });
    }

    /// The double-click test target: a progress ring for the button being tested and a row per button.
    fn draw_dc_test(&self, p: &egui::Painter, pad: Rect, dc: &[(u32, u32); 5], now: f64) {
        p.text(
            pad.center_top() + Vec2::new(0.0, 34.0),
            Align2::CENTER_CENTER,
            format!("Double-click test: click slowly, about once a second, {DC_TARGET} times"),
            theme::bold(18.0),
            Color32::WHITE,
        );
        p.text(
            pad.center_top() + Vec2::new(0.0, 60.0),
            Align2::CENTER_CENTER,
            "Use the button you want to check. A worn switch sometimes clicks twice by itself.",
            egui::FontId::proportional(13.0),
            theme::TEXT_DIM,
        );
        let focus = self.last_button.unwrap_or(0);
        let (n, bad) = dc[focus];
        let r = (pad.height() * 0.2).clamp(60.0, 110.0);
        let c = pad.center() + Vec2::new(0.0, -r * 0.15);
        let flash = (1.0 - (now - self.chatter_at) / 1.0).clamp(0.0, 1.0) as f32;
        let flash = if self.dc_test.is_some() && bad > 0 { flash } else { 0.0 };
        p.circle_filled(c, r, lerp_color(Color32::from_rgba_unmultiplied(16, 16, 19, 230), theme::GLINT, flash * 0.6));
        p.circle_stroke(c, r, Stroke::new(8.0, alpha(Color32::WHITE, 0.08)));
        let frac = (n as f32 / DC_TARGET as f32).min(1.0);
        if frac > 0.0 {
            let steps = 90;
            let pts: Vec<Pos2> = (0..=steps)
                .map(|k| {
                    let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * frac * k as f32 / steps as f32;
                    c + Vec2::new(a.cos(), a.sin()) * r
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(8.0, if bad > 0 { theme::GLINT } else { Color32::WHITE })));
        }
        p.text(c + Vec2::new(0.0, -8.0), Align2::CENTER_CENTER, format!("{n} / {DC_TARGET}"), theme::bold(34.0), Color32::WHITE);
        p.text(c + Vec2::new(0.0, 26.0), Align2::CENTER_CENTER, BUTTONS[focus].1, theme::bold(14.0), theme::TEXT_DIM);
        if flash > 0.0 {
            p.text(c + Vec2::new(0.0, -r - 22.0), Align2::CENTER_CENTER, "Unwanted double-click!", theme::bold(20.0), alpha(Color32::WHITE, flash));
        }

        // One line per button that has been used during the test.
        let mut y = c.y + r + 30.0;
        for i in (0..5).filter(|i| dc[*i].0 + dc[*i].1 > 0) {
            let (n, bad) = dc[i];
            let name = BUTTONS[i].1;
            let (text, col) = match (bad, n >= DC_TARGET) {
                (0, true) => (format!("✔ {name}: no unwanted double-clicks in {n} clicks"), Color32::WHITE),
                (0, false) => (format!("{name}: {n} / {DC_TARGET} clicks, none unwanted so far"), theme::TEXT),
                (b, true) => (format!("⚠ {name}: {} in {}: the switch may be wearing out", plural(b, "unwanted double-click"), plural(n, "click")), theme::GLINT),
                (b, false) => (format!("⚠ {name}: {} in {} so far", plural(b, "unwanted double-click"), plural(n, "click")), theme::GLINT),
            };
            p.text(Pos2::new(c.x, y), Align2::CENTER_CENTER, text, theme::bold(14.0), col);
            y += 22.0;
        }
    }

    fn draw_mouse(&self, ui: &egui::Ui, area: Rect, now: f64) {
        let p = ui.painter();
        let ctx = ui.ctx();
        let body = Rect::from_center_size(area.center() + Vec2::new(10.0, 0.0), Vec2::new(150.0, 250.0));
        p.rect_filled(body.translate(Vec2::new(0.0, 6.0)), 70.0, Color32::from_black_alpha(200));
        theme::vgradient(p, body, 70.0, Color32::from_rgb(52, 52, 56), Color32::from_rgb(24, 24, 27));
        p.rect_stroke(body, 70.0, Stroke::new(1.5, alpha(Color32::WHITE, 0.25)), StrokeKind::Inside);

        let split = body.top() + body.height() * 0.42;
        let mid = body.center().x;
        let left = Rect::from_min_max(body.left_top() + Vec2::new(8.0, 8.0), Pos2::new(mid - 3.0, split));
        let right = Rect::from_min_max(Pos2::new(mid + 3.0, body.top() + 8.0), Pos2::new(body.right() - 8.0, split));
        let wheel = Rect::from_center_size(Pos2::new(mid, body.top() + body.height() * 0.22), Vec2::new(22.0, 50.0));
        let back = Rect::from_min_size(Pos2::new(body.left() - 12.0, split + 18.0), Vec2::new(14.0, 34.0));
        let fwd = Rect::from_min_size(Pos2::new(body.left() - 12.0, split - 22.0), Vec2::new(14.0, 34.0));

        let part = |rect: Rect, r: f32, b: PointerButton, label: &str| {
            let i = idx(b).unwrap_or(0);
            let held = ctx.animate_bool_with_time(egui::Id::unique(("mb-h", label)), self.down[i], 0.08);
            let ok = ctx.animate_bool_with_time(egui::Id::unique(("mb-ok", label)), self.tested[i], 0.4);
            if held > 0.01 {
                theme::rect_glow(p, rect, r, Color32::WHITE, held);
            }
            let top = lerp_color(lerp_color(Color32::from_rgb(64, 64, 68), Color32::from_rgb(140, 140, 146), ok), Color32::WHITE, held);
            let bot = lerp_color(lerp_color(Color32::from_rgb(36, 36, 40), Color32::from_rgb(90, 90, 96), ok), Color32::from_rgb(205, 205, 210), held);
            theme::vgradient(p, rect, r, top, bot);
            p.rect_stroke(rect, r, Stroke::new(1.0, alpha(Color32::WHITE, 0.2 + 0.4 * ok)), StrokeKind::Inside);
            ok.max(held)
        };
        let lit = part(left, 30.0, PointerButton::Primary, "L");
        p.text(left.center() + Vec2::new(-6.0, 6.0), Align2::CENTER_CENTER, "L", theme::bold(16.0), lerp_color(theme::TEXT_DIM, theme::INK, lit));
        let lit = part(right, 30.0, PointerButton::Secondary, "R");
        p.text(right.center() + Vec2::new(6.0, 6.0), Align2::CENTER_CENTER, "R", theme::bold(16.0), lerp_color(theme::TEXT_DIM, theme::INK, lit));
        part(wheel, 11.0, PointerButton::Middle, "M");
        part(back, 5.0, PointerButton::Extra1, "B");
        part(fwd, 5.0, PointerButton::Extra2, "F");

        // Scroll arrows around the wheel light up briefly when used, and stay silver once tested.
        let arrow = |dir: Scroll, at: Pos2, glyph: &str| {
            let since = self.scroll_at.get(&dir).map_or(9.0, |t| now - t) as f32;
            let flash = (1.0 - since / 0.4).clamp(0.0, 1.0);
            let base = if self.scrolled.contains(&dir) { theme::SILVER } else { alpha(theme::TEXT_DIM, 0.6) };
            p.text(at, Align2::CENTER_CENTER, glyph, theme::bold(15.0 + 5.0 * flash), lerp_color(base, Color32::WHITE, flash));
        };
        arrow(Scroll::Up, wheel.center_top() + Vec2::new(0.0, -14.0), "↑");
        arrow(Scroll::Down, wheel.center_bottom() + Vec2::new(0.0, 14.0), "↓");
        arrow(Scroll::Left, wheel.left_center() + Vec2::new(-16.0, 0.0), "←");
        arrow(Scroll::Right, wheel.right_center() + Vec2::new(16.0, 0.0), "→");

        // Small labels for the side buttons.
        p.text(fwd.left_center() + Vec2::new(-6.0, 0.0), Align2::RIGHT_CENTER, "Fwd", egui::FontId::proportional(11.0), theme::TEXT_DIM);
        p.text(back.left_center() + Vec2::new(-6.0, 0.0), Align2::RIGHT_CENTER, "Back", egui::FontId::proportional(11.0), theme::TEXT_DIM);
    }
}

fn plural(n: u32, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

/// A gap between clicks, rounded honestly: input arrives once per frame (~16 ms).
fn fmt_gap(secs: f64) -> String {
    let ms = secs * 1000.0;
    if ms < 16.0 {
        "<16 ms".into()
    } else if ms < 1000.0 {
        format!("~{} ms", ms.round() as u32)
    } else if ms < 10_000.0 {
        format!("{:.1} s", secs)
    } else {
        ">10 s".into()
    }
}

fn chip(ui: &mut egui::Ui, label: &str, ok: bool) {
    egui::Frame::new()
        .fill(if ok { Color32::WHITE } else { alpha(theme::SILVER, 0.15) })
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(9, 3))
        .show(ui, |ui| {
            let text = if ok { format!("✔ {label}") } else { label.to_string() };
            ui.label(egui::RichText::new(text).size(12.5).color(if ok { theme::INK } else { theme::TEXT_DIM }));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_buttons_never_make_a_double_click() {
        let mut m = MouseTest::default();
        m.press(2, 1.0); // Right
        m.press(0, 1.1); // Left soon after
        assert!(!m.double_clicked);
        assert_eq!(m.streak[0], 1);
    }

    #[test]
    fn slow_clicks_are_singles() {
        let mut m = MouseTest::default();
        m.press(0, 1.0);
        m.press(0, 1.8);
        assert!(!m.double_clicked);
        assert_eq!(m.clicks[0], 2);
    }

    #[test]
    fn two_quick_left_clicks_are_a_double_and_three_a_triple() {
        let mut m = MouseTest::default();
        m.press(0, 1.0);
        m.press(0, 1.2);
        assert!(m.double_clicked);
        m.press(0, 1.4);
        assert_eq!(m.streak[0], 3);
    }

    #[test]
    fn gaps_are_recorded_and_rounded() {
        let mut m = MouseTest::default();
        m.press(0, 1.0);
        m.press(0, 1.8);
        m.press(0, 1.845); // 45 ms: chatter with the 50 ms threshold
        assert_eq!(m.chatter[0], 1);
        assert_eq!(m.gaps[0].len(), 2);
        assert_eq!(fmt_gap(m.min_gap[0].unwrap()), "~45 ms");
        assert_eq!(fmt_gap(0.0), "<16 ms");
        assert_eq!(fmt_gap(0.812), "~812 ms");
        assert_eq!(fmt_gap(2.0), "2.0 s");
    }

    #[test]
    fn double_click_test_counts_from_its_start() {
        let mut m = MouseTest::default();
        m.press(0, 1.0);
        m.press(0, 1.01); // chatter before the test: not counted in it
        m.start_dc_test();
        m.press(0, 3.0);
        m.press(0, 4.0);
        m.press(0, 4.02); // unwanted double-click during the test
        assert_eq!(m.dc_counts().unwrap()[0], (2, 1));
    }

    #[test]
    fn chatter_is_not_a_click() {
        let mut m = MouseTest::default();
        m.press(0, 1.0);
        m.press(0, 1.01);
        assert_eq!((m.clicks[0], m.chatter[0]), (1, 1));
        assert!(!m.double_clicked);
    }
}
