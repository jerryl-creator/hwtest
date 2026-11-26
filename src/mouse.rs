//! Mouse / touchpad test: buttons, scroll directions, double-click, click chatter and a drawing area.

use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};

/// Two presses of the same button closer than this are a faulty switch, not a person.
const CLICK_CHATTER_SECS: f64 = 0.04;
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
    pub double_clicked: bool,
    double_at: f64,
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
                    if self.last_press[i].replace(now).is_some_and(|prev| now - prev < CLICK_CHATTER_SECS) {
                        self.chatter[i] += 1;
                    }
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
        lines.push(format!("Double-click: {}", if self.double_clicked { "works" } else { "not tested" }));
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

        if let Some(i) = worst {
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
                });
            });
            ui.label(
                egui::RichText::new("Click each button, scroll in every direction, double-click, and move around the drawing area.")
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);

            // Fill the window: the card's other rows need about 190 px (+30 with the chatter line).
            let reserved = if self.chatter.iter().any(|c| *c > 0) { 222.0 } else { 190.0 };
            let h = (self.fit_height - reserved).clamp(300.0, 700.0);
            ui.horizontal_top(|ui| {
                // Mouse diagram
                let (area, _) = ui.allocate_exact_size(Vec2::new(230.0, h), Sense::hover());
                self.draw_mouse(ui, area, now);

                ui.add_space(16.0);
                // Drawing / double-click area
                let (pad, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click_and_drag());
                if resp.double_clicked() {
                    self.double_clicked = true;
                    self.double_at = now;
                }
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
                for w in self.trail.iter().collect::<Vec<_>>().windows(2) {
                    let a = (1.0 - (now - w[1].1) / TRAIL_SECS) as f32;
                    p.line_segment([w[0].0, w[1].0], Stroke::new(1.0 + 3.0 * a, alpha(Color32::WHITE, a)));
                }
                if self.trail.is_empty() {
                    p.text(pad.center(), Align2::CENTER_CENTER, "Move, click and double-click here", theme::bold(16.0), alpha(theme::TEXT_DIM, 0.8));
                }
                let since = now - self.double_at;
                if self.double_clicked && since < 1.0 {
                    let a = (1.0 - since) as f32;
                    p.text(pad.center_top() + Vec2::new(0.0, 34.0), Align2::CENTER_CENTER, "Double-click! ✔", theme::bold(22.0), alpha(Color32::WHITE, a));
                }
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                for (i, (_, name)) in BUTTONS.iter().enumerate() {
                    chip(ui, name, self.tested[i]);
                }
                ui.add_space(8.0);
                for (d, name) in [(Scroll::Up, "Scroll ↑"), (Scroll::Down, "Scroll ↓"), (Scroll::Left, "Scroll ←"), (Scroll::Right, "Scroll →")] {
                    chip(ui, name, self.scrolled.contains(&d));
                }
                ui.add_space(8.0);
                chip(ui, "Double-click", self.double_clicked);
            });
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
