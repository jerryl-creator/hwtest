//! Gamepad test: buttons, D-pad, triggers, sticks (with drift detection) and rumble.

use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use gilrs::{Axis, Button, GamepadId, Gilrs};
use std::collections::HashSet;

/// The buttons we test, with their label on an Xbox-style pad.
pub const BUTTONS: [(Button, &str); 17] = [
    (Button::South, "A"),
    (Button::East, "B"),
    (Button::West, "X"),
    (Button::North, "Y"),
    (Button::DPadUp, "D-pad ↑"),
    (Button::DPadDown, "D-pad ↓"),
    (Button::DPadLeft, "D-pad ←"),
    (Button::DPadRight, "D-pad →"),
    (Button::LeftTrigger, "LB"),
    (Button::RightTrigger, "RB"),
    (Button::LeftTrigger2, "LT"),
    (Button::RightTrigger2, "RT"),
    (Button::Select, "View"),
    (Button::Start, "Menu"),
    (Button::Mode, "Guide"),
    (Button::LeftThumb, "L-stick click"),
    (Button::RightThumb, "R-stick click"),
];

/// Values closer to the centre than this count as "at rest".
const DEADZONE: f32 = 0.10;

/// Watches one stick for drift: sitting still but off-centre.
#[derive(Default, Clone, Copy)]
struct StickWatch {
    last: Vec2,
    still_since: f64,
    /// Furthest the stick has been pushed (0..~1.4).
    pub max_reach: f32,
    /// Off-centre rest position seen (drift), if any.
    pub drift: Option<Vec2>,
}

impl StickWatch {
    fn update(&mut self, v: Vec2, now: f64) {
        self.max_reach = self.max_reach.max(v.length());
        if (v - self.last).length() > 0.02 {
            self.last = v;
            self.still_since = now;
        } else if now - self.still_since > 1.2 {
            let off = v.length();
            if off > DEADZONE && off < 0.6 {
                self.drift = Some(v);
            } else if off <= DEADZONE * 0.5 {
                // Back in the middle: it settled properly.
                self.drift = None;
            }
        }
    }
}

pub struct GamepadTest {
    gilrs: Option<Gilrs>,
    error: Option<String>,
    active: Option<GamepadId>,
    pub tested: HashSet<Button>,
    left: StickWatch,
    right: StickWatch,
    pub rumble_tried: bool,
    /// The last rumble effect (kept alive while it plays; dropping it stops it).
    effect: Option<gilrs::ff::Effect>,
    /// Name of the pad being tested (for the report).
    pub name: Option<String>,
}

impl Default for GamepadTest {
    fn default() -> Self {
        let (gilrs, error) = match Gilrs::new() {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self { gilrs, error, active: None, tested: HashSet::new(), left: Default::default(), right: Default::default(), rumble_tried: false, effect: None, name: None }
    }
}

impl GamepadTest {
    /// Read controller events. Called every frame, whatever tab is showing.
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(g) = self.gilrs.as_mut() else { return };
        let now = ctx.input(|i| i.time);
        let mut any = false;
        while let Some(ev) = g.next_event() {
            any = true;
            // Follow whichever pad was used last.
            if self.active != Some(ev.id) {
                self.active = Some(ev.id);
                self.name = Some(g.gamepad(ev.id).name().to_string());
            }
            if let gilrs::EventType::ButtonPressed(b, _) = ev.event {
                self.tested.insert(b);
            }
            if let gilrs::EventType::ButtonChanged(b, v, _) = ev.event {
                if v > 0.5 {
                    self.tested.insert(b);
                }
            }
        }
        if self.active.is_none() {
            if let Some((id, pad)) = g.gamepads().next() {
                self.active = Some(id);
                self.name = Some(pad.name().to_string());
            }
        }
        if let Some(pad) = self.active.and_then(|id| g.connected_gamepad(id)) {
            let l = Vec2::new(pad.value(Axis::LeftStickX), -pad.value(Axis::LeftStickY));
            let r = Vec2::new(pad.value(Axis::RightStickX), -pad.value(Axis::RightStickY));
            self.left.update(l, now);
            self.right.update(r, now);
        }
        if any {
            ctx.request_repaint();
        }
    }

    pub fn left_ok(&self) -> bool {
        self.left.max_reach > 0.9
    }
    pub fn right_ok(&self) -> bool {
        self.right.max_reach > 0.9
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        let done = BUTTONS.iter().filter(|(b, _)| self.tested.contains(b)).count();
        let drift = self.left.drift.is_some() || self.right.drift.is_some();
        let status = if drift {
            Status::Issue
        } else if self.name.is_none() || (done == 0 && self.left.max_reach < 0.05 && self.right.max_reach < 0.05) {
            Status::NotTested
        } else if done == BUTTONS.len() && self.left_ok() && self.right_ok() {
            Status::Pass
        } else {
            Status::InProgress
        };
        let mut lines = Vec::new();
        if let Some(n) = &self.name {
            lines.push(format!("Controller: {n}"));
        }
        lines.push(format!("{done} / {} buttons work", BUTTONS.len()));
        if done > 0 && done < BUTTONS.len() {
            let missing: Vec<&str> = BUTTONS.iter().filter(|(b, _)| !self.tested.contains(b)).map(|(_, n)| *n).collect();
            lines.push(format!("Not pressed yet: {}", missing.join(", ")));
        }
        for (side, w) in [("Left", &self.left), ("Right", &self.right)] {
            let range = if w.max_reach > 0.9 { "full range" } else if w.max_reach > 0.05 { "not pushed to the edge yet" } else { "not tested" };
            let drift = w.drift.map_or(String::new(), |v| format!(", DRIFT: rests {:.0}% off-centre", v.length() * 100.0));
            lines.push(format!("{side} stick: {range}{drift}"));
        }
        if self.rumble_tried {
            lines.push("Rumble: tried".into());
        }
        Section { title: "Gamepad", jp: "ゲームパッド", status, lines }
    }

    pub fn reset(&mut self) {
        self.tested.clear();
        self.left = Default::default();
        self.right = Default::default();
        self.rumble_tried = false;
    }

    fn rumble(&mut self) {
        let (Some(g), Some(id)) = (self.gilrs.as_mut(), self.active) else { return };
        use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
        let effect = EffectBuilder::new()
            .add_effect(BaseEffect {
                kind: BaseEffectType::Strong { magnitude: 45_000 },
                scheduling: Replay { play_for: Ticks::from_ms(500), ..Default::default() },
                ..Default::default()
            })
            .add_effect(BaseEffect {
                kind: BaseEffectType::Weak { magnitude: 45_000 },
                scheduling: Replay { after: Ticks::from_ms(550), play_for: Ticks::from_ms(400), ..Default::default() },
                ..Default::default()
            })
            .gamepads(&[id])
            .finish(g);
        if let Ok(e) = effect {
            let _ = e.play();
            self.effect = Some(e);
            self.rumble_tried = true;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        let now = ui.input(|i| i.time);
        let pad = self.gilrs.as_ref().and_then(|g| self.active.and_then(|id| g.connected_gamepad(id)));
        let connected = pad.is_some();
        let ff = pad.as_ref().is_some_and(|p| p.is_ff_supported());
        let pressed: HashSet<Button> = pad
            .as_ref()
            .map(|p| BUTTONS.iter().map(|(b, _)| *b).filter(|b| p.is_pressed(*b)).collect())
            .unwrap_or_default();
        let trig = |b: Button| pad.as_ref().and_then(|p| p.button_data(b)).map_or(0.0, |d| d.value());
        let (lt, rt) = (trig(Button::LeftTrigger2), trig(Button::RightTrigger2));
        let stick = |x: Axis, y: Axis| pad.as_ref().map_or(Vec2::ZERO, |p| Vec2::new(p.value(x), -p.value(y)));
        let (ls, rs) = (stick(Axis::LeftStickX, Axis::LeftStickY), stick(Axis::RightStickX, Axis::RightStickY));
        let done = BUTTONS.iter().filter(|(b, _)| self.tested.contains(b)).count();

        if self.left.drift.is_some() || self.right.drift.is_some() {
            mascot.say("A stick isn't centring… stick drift! (；ω；)");
        } else if !connected {
            mascot.say("Plug in or pair a controller, then press a button~");
        } else if done == BUTTONS.len() && self.left_ok() && self.right_ok() {
            mascot.say("Your controller is perfect! ＼(^o^)／");
        } else {
            mascot.say("Press every button and roll both sticks around~ ♪");
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Gamepad", "ゲームパッド", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::fancy_button(ui, "↺ Reset", Vec2::new(100.0, 32.0), theme::SILVER, theme::STEEL, true).clicked() {
                        self.reset();
                    }
                    ui.add_space(6.0);
                    let rumble = theme::fancy_button(ui, "〰 Rumble", Vec2::new(120.0, 32.0), theme::ACCENT, theme::SILVER, ff)
                        .on_hover_text(if ff { "Vibrate the controller" } else { "This controller doesn't support vibration here" });
                    if rumble.clicked() {
                        self.rumble();
                    }
                    ui.add_space(10.0);
                    let label = match (&self.error, &self.name, connected) {
                        (Some(e), _, _) => format!("Controllers unavailable: {e}"),
                        (_, Some(n), true) => n.clone(),
                        _ => "No controller connected".to_string(),
                    };
                    ui.label(egui::RichText::new(label).size(12.5).color(theme::TEXT_DIM));
                });
            });

            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 380.0), Sense::hover());
            self.draw_pad(ui, area, &pressed, lt, rt, ls, rs, connected, now);

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{done} / {} buttons", BUTTONS.len())).font(theme::bold(13.0)).color(Color32::WHITE));
                ui.add_space(6.0);
                for (b, name) in BUTTONS {
                    chip(ui, name, self.tested.contains(&b));
                }
                chip(ui, "Left stick full range", self.left_ok());
                chip(ui, "Right stick full range", self.right_ok());
            });
            for (name, d) in [("Left", self.left.drift), ("Right", self.right.drift)] {
                if let Some(v) = d {
                    ui.label(
                        egui::RichText::new(format!(
                            "⚠ {name} stick drift: it rests at {:.0}% off-centre instead of the middle. Games may move on their own.",
                            v.length() * 100.0
                        ))
                        .font(theme::bold(13.0))
                        .color(Color32::WHITE),
                    );
                }
            }
            ui.label(
                egui::RichText::new(
                    "Labels follow an Xbox pad (PlayStation: A = Cross, B = Circle, X = Square, Y = Triangle). \
                     Let go of both sticks to check for drift. The Guide button may be grabbed by Steam or the Xbox Game Bar.",
                )
                .size(12.0)
                .color(theme::TEXT_DIM),
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pad(&self, ui: &egui::Ui, area: Rect, pressed: &HashSet<Button>, lt: f32, rt: f32, ls: Vec2, rs: Vec2, connected: bool, now: f64) {
        let p = ui.painter();
        let ctx = ui.ctx();
        let c = area.center() + Vec2::new(0.0, 10.0);
        let s = (area.height() / 380.0).min(area.width() / 700.0).max(0.6);

        // Body: a wide rounded centre plus two grips.
        let body_col = (Color32::from_rgb(52, 52, 56), Color32::from_rgb(22, 22, 25));
        for grip in [-1.0f32, 1.0] {
            let g = Rect::from_center_size(c + Vec2::new(grip * 170.0 * s, 60.0 * s), Vec2::new(150.0 * s, 220.0 * s));
            theme::vgradient(p, g, 70.0 * s, body_col.0, body_col.1);
        }
        let body = Rect::from_center_size(c, Vec2::new(480.0 * s, 210.0 * s));
        theme::vgradient(p, body, 90.0 * s, body_col.0, body_col.1);
        p.rect_stroke(body, 90.0 * s, Stroke::new(1.5, alpha(Color32::WHITE, 0.18)), StrokeKind::Inside);
        if !connected {
            p.text(c + Vec2::new(0.0, -8.0 * s), Align2::CENTER_CENTER, "No controller", theme::bold(18.0), alpha(theme::TEXT_DIM, 0.9));
        }

        // A key-like part: held = white glow, tested = silver, else dark.
        let part = |rect: Rect, r: f32, b: Button, label: &str| {
            let held = ctx.animate_bool_with_time(egui::Id::unique(("gp-h", b as u32)), pressed.contains(&b), 0.08);
            let ok = ctx.animate_bool_with_time(egui::Id::unique(("gp-ok", b as u32)), self.tested.contains(&b), 0.4);
            if held > 0.01 {
                theme::rect_glow(p, rect, r, Color32::WHITE, held);
            }
            let top = lerp_color(lerp_color(Color32::from_rgb(70, 70, 75), Color32::from_rgb(145, 145, 150), ok), Color32::WHITE, held);
            let bot = lerp_color(lerp_color(Color32::from_rgb(36, 36, 40), Color32::from_rgb(92, 92, 98), ok), Color32::from_rgb(205, 205, 210), held);
            theme::vgradient(p, rect, r, top, bot);
            p.rect_stroke(rect, r, Stroke::new(1.0, alpha(Color32::WHITE, 0.2 + 0.4 * ok)), StrokeKind::Inside);
            let txt = lerp_color(lerp_color(theme::TEXT_DIM, Color32::WHITE, ok), theme::INK, held);
            p.text(rect.center(), Align2::CENTER_CENTER, label, theme::bold((13.0 * s).max(10.0)), txt);
        };

        // Face buttons (right) in a diamond.
        let face = c + Vec2::new(165.0 * s, -30.0 * s);
        let fb = 34.0 * s;
        for (b, label, off) in [
            (Button::North, "Y", Vec2::new(0.0, -38.0)),
            (Button::South, "A", Vec2::new(0.0, 38.0)),
            (Button::West, "X", Vec2::new(-38.0, 0.0)),
            (Button::East, "B", Vec2::new(38.0, 0.0)),
        ] {
            part(Rect::from_center_size(face + off * s, Vec2::splat(fb)), fb / 2.0, b, label);
        }
        // D-pad (left).
        let dp = c + Vec2::new(-80.0 * s, 50.0 * s);
        let db = 28.0 * s;
        for (b, label, off) in [
            (Button::DPadUp, "↑", Vec2::new(0.0, -30.0)),
            (Button::DPadDown, "↓", Vec2::new(0.0, 30.0)),
            (Button::DPadLeft, "←", Vec2::new(-30.0, 0.0)),
            (Button::DPadRight, "→", Vec2::new(30.0, 0.0)),
        ] {
            part(Rect::from_center_size(dp + off * s, Vec2::splat(db)), 5.0, b, label);
        }
        // Small centre buttons.
        part(Rect::from_center_size(c + Vec2::new(-45.0 * s, -45.0 * s), Vec2::new(40.0 * s, 20.0 * s)), 10.0 * s, Button::Select, "View");
        part(Rect::from_center_size(c + Vec2::new(45.0 * s, -45.0 * s), Vec2::new(40.0 * s, 20.0 * s)), 10.0 * s, Button::Start, "Menu");
        part(Rect::from_center_size(c + Vec2::new(0.0, -78.0 * s), Vec2::splat(30.0 * s)), 15.0 * s, Button::Mode, "⌂");
        // Bumpers and trigger bars on top.
        for (side, bump, bl, trig, tl, value) in [
            (-1.0f32, Button::LeftTrigger, "LB", Button::LeftTrigger2, "LT", lt),
            (1.0, Button::RightTrigger, "RB", Button::RightTrigger2, "RT", rt),
        ] {
            let x = c.x + side * 170.0 * s;
            part(Rect::from_center_size(Pos2::new(x, c.y - 118.0 * s), Vec2::new(110.0 * s, 22.0 * s)), 11.0 * s, bump, bl);
            let bar = Rect::from_center_size(Pos2::new(x, c.y - 152.0 * s), Vec2::new(110.0 * s, 22.0 * s));
            let ok = self.tested.contains(&trig);
            p.rect_filled(bar, 11.0 * s, Color32::from_rgba_unmultiplied(10, 10, 12, 220));
            if value > 0.01 {
                let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * value.clamp(0.0, 1.0), bar.height()));
                theme::hgradient(p, fill, 11.0 * s, theme::STEEL, Color32::WHITE);
            }
            p.rect_stroke(bar, 11.0 * s, Stroke::new(1.0, alpha(Color32::WHITE, if ok { 0.6 } else { 0.2 })), StrokeKind::Inside);
            let txt = format!("{tl} {:.0}%", value * 100.0);
            p.text(bar.center(), Align2::CENTER_CENTER, txt, theme::bold((12.0 * s).max(10.0)), if value > 0.5 { theme::INK } else { Color32::WHITE });
        }
        // Sticks: outer ring, deadzone circle, live dot, and a ring that lights when clicked.
        for (centre, v, watch, click) in [
            (c + Vec2::new(-165.0 * s, -30.0 * s), ls, &self.left, Button::LeftThumb),
            (c + Vec2::new(80.0 * s, 50.0 * s), rs, &self.right, Button::RightThumb),
        ] {
            let r = 42.0 * s;
            let clicked = pressed.contains(&click);
            p.circle_filled(centre, r, Color32::from_rgba_unmultiplied(8, 8, 10, 230));
            let ring = if clicked { Color32::WHITE } else if self.tested.contains(&click) { theme::SILVER } else { alpha(Color32::WHITE, 0.25) };
            p.circle_stroke(centre, r, Stroke::new(if clicked { 3.0 } else { 1.5 }, ring));
            p.circle_stroke(centre, r * DEADZONE, Stroke::new(1.0, alpha(Color32::WHITE, 0.35)));
            // Max reach so far.
            if watch.max_reach > 0.05 {
                p.circle_stroke(centre, r * watch.max_reach.min(1.0), Stroke::new(1.0, alpha(theme::SILVER, 0.5)));
            }
            let dot = centre + Vec2::new(v.x.clamp(-1.0, 1.0), v.y.clamp(-1.0, 1.0)) * r;
            theme::glow(p, dot, 16.0 * s, alpha(Color32::WHITE, 0.35));
            let drift = watch.drift.is_some();
            let pulse = if drift { 1.0 + 0.25 * ((now * 6.0).sin() as f32) } else { 1.0 };
            p.circle_filled(dot, 8.0 * s * pulse, if drift { theme::GLINT } else { Color32::WHITE });
            if drift {
                p.text(centre + Vec2::new(0.0, r + 14.0), Align2::CENTER_CENTER, "⚠ drift", theme::bold(12.0), Color32::WHITE);
            }
        }
    }
}

fn chip(ui: &mut egui::Ui, label: &str, ok: bool) {
    egui::Frame::new()
        .fill(if ok { Color32::WHITE } else { alpha(theme::SILVER, 0.15) })
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(8, 2))
        .show(ui, |ui| {
            let text = if ok { format!("✔ {label}") } else { label.to_string() };
            ui.label(egui::RichText::new(text).size(12.0).color(if ok { theme::INK } else { theme::TEXT_DIM }));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centred_stick_has_no_drift() {
        let mut w = StickWatch::default();
        for i in 0..200 {
            w.update(Vec2::new(0.02, -0.01), i as f64 * 0.016);
        }
        assert!(w.drift.is_none());
    }

    #[test]
    fn stick_resting_off_centre_is_drift() {
        let mut w = StickWatch::default();
        for i in 0..200 {
            w.update(Vec2::new(0.18, 0.05), i as f64 * 0.016);
        }
        assert!(w.drift.is_some());
    }

    #[test]
    fn holding_a_stick_far_over_is_not_drift() {
        let mut w = StickWatch::default();
        for i in 0..200 {
            w.update(Vec2::new(0.95, 0.0), i as f64 * 0.016);
        }
        assert!(w.drift.is_none());
        assert!(w.max_reach > 0.9);
    }
}
