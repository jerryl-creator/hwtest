//! Gamepad test: buttons, D-pad, triggers, sticks (with drift detection) and rumble.

use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use gilrs::{Axis, Button, GamepadId, Gilrs};
use std::collections::HashSet;

/// The buttons we test. gilrs names them by position (South = bottom face button).
pub const BUTTONS: [Button; 17] = [
    Button::South,
    Button::East,
    Button::West,
    Button::North,
    Button::DPadUp,
    Button::DPadDown,
    Button::DPadLeft,
    Button::DPadRight,
    Button::LeftTrigger,
    Button::RightTrigger,
    Button::LeftTrigger2,
    Button::RightTrigger2,
    Button::Select,
    Button::Start,
    Button::Mode,
    Button::LeftThumb,
    Button::RightThumb,
];

/// Which controller family to draw and label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Xbox,
    Ps3,
    Ps4,
    Ps5,
    Nintendo,
}

impl Style {
    pub fn name(self) -> &'static str {
        match self {
            Style::Xbox => "Xbox",
            Style::Ps3 => "PlayStation 3",
            Style::Ps4 => "PlayStation 4",
            Style::Ps5 => "PlayStation 5",
            Style::Nintendo => "Nintendo Switch",
        }
    }
    fn is_ps(self) -> bool {
        matches!(self, Style::Ps3 | Style::Ps4 | Style::Ps5)
    }
}

/// The layout picker: Auto follows the connected controller.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    Auto,
    Xbox,
    PlayStation,
    Nintendo,
}

/// Guess the controller family from its USB IDs, falling back to its name.
pub fn detect(vendor: Option<u16>, product: Option<u16>, name: &str) -> Style {
    let n = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if vendor == Some(0x054C) || has(&["playstation", "dualshock", "dualsense", "ps3", "ps4", "ps5", "sony"]) {
        return match product {
            Some(0x0268) => Style::Ps3,
            Some(0x05C4 | 0x09CC | 0x0BA0) => Style::Ps4,
            Some(0x0CE6 | 0x0DF2) => Style::Ps5,
            _ if has(&["ps3", "playstation(r)3", "playstation 3", "dualshock 3", "sixaxis"]) => Style::Ps3,
            _ if has(&["ps5", "dualsense"]) => Style::Ps5,
            _ => Style::Ps4,
        };
    }
    if vendor == Some(0x057E) || has(&["nintendo", "pro controller", "joy-con"]) {
        return Style::Nintendo;
    }
    Style::Xbox
}

/// What a button is called on this kind of controller.
pub fn label(style: Style, b: Button) -> &'static str {
    use Button as B;
    match (b, style) {
        (B::DPadUp, _) => "D-pad ↑",
        (B::DPadDown, _) => "D-pad ↓",
        (B::DPadLeft, _) => "D-pad ←",
        (B::DPadRight, _) => "D-pad →",
        (B::South, Style::Xbox) => "A",
        (B::East, Style::Xbox) => "B",
        (B::West, Style::Xbox) => "X",
        (B::North, Style::Xbox) => "Y",
        (B::LeftTrigger, Style::Xbox) => "LB",
        (B::RightTrigger, Style::Xbox) => "RB",
        (B::LeftTrigger2, Style::Xbox) => "LT",
        (B::RightTrigger2, Style::Xbox) => "RT",
        (B::Select, Style::Xbox) => "View",
        (B::Start, Style::Xbox) => "Menu",
        (B::Mode, Style::Xbox) => "Guide",
        (B::South, Style::Nintendo) => "B",
        (B::East, Style::Nintendo) => "A",
        (B::West, Style::Nintendo) => "Y",
        (B::North, Style::Nintendo) => "X",
        (B::LeftTrigger, Style::Nintendo) => "L",
        (B::RightTrigger, Style::Nintendo) => "R",
        (B::LeftTrigger2, Style::Nintendo) => "ZL",
        (B::RightTrigger2, Style::Nintendo) => "ZR",
        (B::Select, Style::Nintendo) => "−",
        (B::Start, Style::Nintendo) => "+",
        (B::Mode, Style::Nintendo) => "Home",
        (B::South, _) => "Cross",
        (B::East, _) => "Circle",
        (B::West, _) => "Square",
        (B::North, _) => "Triangle",
        (B::LeftTrigger, _) => "L1",
        (B::RightTrigger, _) => "R1",
        (B::LeftTrigger2, _) => "L2",
        (B::RightTrigger2, _) => "R2",
        (B::Select, Style::Ps3) => "Select",
        (B::Select, Style::Ps4) => "Share",
        (B::Select, _) => "Create",
        (B::Start, Style::Ps3) => "Start",
        (B::Start, _) => "Options",
        (B::Mode, _) => "PS",
        (B::LeftThumb, s) if s.is_ps() => "L3",
        (B::RightThumb, s) if s.is_ps() => "R3",
        (B::LeftThumb, _) => "L-stick click",
        (B::RightThumb, _) => "R-stick click",
        _ => "?",
    }
}

/// Draw a PlayStation face-button symbol (drawn, so it works with every font).
fn ps_symbol(p: &egui::Painter, c: Pos2, size: f32, b: Button, col: Color32) {
    let r = size * 0.28;
    let st = Stroke::new((size * 0.07).max(1.5), col);
    match b {
        Button::North => {
            let pts = vec![c + Vec2::new(0.0, -r * 1.05), c + Vec2::new(r * 1.0, r * 0.75), c + Vec2::new(-r * 1.0, r * 0.75)];
            p.add(egui::Shape::closed_line(pts, st));
        }
        Button::East => {
            p.circle_stroke(c, r, st);
        }
        Button::South => {
            p.line_segment([c + Vec2::splat(-r), c + Vec2::splat(r)], st);
            p.line_segment([c + Vec2::new(r, -r), c + Vec2::new(-r, r)], st);
        }
        _ => {
            p.rect_stroke(Rect::from_center_size(c, Vec2::splat(r * 1.8)), 1.0, st, StrokeKind::Middle);
        }
    }
}

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
    /// Family guessed from the connected pad.
    detected: Style,
    /// What the layout picker is set to.
    pub choice: Choice,
}

impl Default for GamepadTest {
    fn default() -> Self {
        let (gilrs, error) = match Gilrs::new() {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self { gilrs, error, active: None, tested: HashSet::new(), left: Default::default(), right: Default::default(), rumble_tried: false, effect: None, name: None, detected: Style::Xbox, choice: dev_choice() }
    }
}

/// NYANSPECT_PAD=ps3|ps4|ps5|xbox|nintendo forces a layout (for screenshots).
fn dev_choice() -> Choice {
    match std::env::var("NYANSPECT_PAD").as_deref() {
        Ok("xbox") => Choice::Xbox,
        Ok("ps3" | "ps4" | "ps5" | "playstation") => Choice::PlayStation,
        Ok("nintendo") => Choice::Nintendo,
        _ => Choice::Auto,
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
                let pad = g.gamepad(ev.id);
                self.name = Some(pad.name().to_string());
                self.detected = detect(pad.vendor_id(), pad.product_id(), pad.name());
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
                self.detected = detect(pad.vendor_id(), pad.product_id(), pad.name());
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

    /// The layout being shown: the picker's choice, or what was detected.
    pub fn style(&self) -> Style {
        let forced = match std::env::var("NYANSPECT_PAD").as_deref() {
            Ok("ps3") => Some(Style::Ps3),
            Ok("ps5") => Some(Style::Ps5),
            _ => None,
        };
        match self.choice {
            Choice::Auto => self.detected,
            Choice::Xbox => Style::Xbox,
            Choice::Nintendo => Style::Nintendo,
            Choice::PlayStation => forced.unwrap_or(if self.detected.is_ps() { self.detected } else { Style::Ps4 }),
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
        let style = self.style();
        let done = BUTTONS.iter().filter(|b| self.tested.contains(b)).count();
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
            lines.push(format!("Controller: {n} ({} layout)", style.name()));
        }
        lines.push(format!("{done} / {} buttons work", BUTTONS.len()));
        if done > 0 && done < BUTTONS.len() {
            let missing: Vec<&str> = BUTTONS.iter().filter(|b| !self.tested.contains(b)).map(|b| label(style, *b)).collect();
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
            .map(|p| BUTTONS.iter().copied().filter(|b| p.is_pressed(*b)).collect())
            .unwrap_or_default();
        let trig = |b: Button| pad.as_ref().and_then(|p| p.button_data(b)).map_or(0.0, |d| d.value());
        let (lt, rt) = (trig(Button::LeftTrigger2), trig(Button::RightTrigger2));
        let stick = |x: Axis, y: Axis| pad.as_ref().map_or(Vec2::ZERO, |p| Vec2::new(p.value(x), -p.value(y)));
        let (ls, rs) = (stick(Axis::LeftStickX, Axis::LeftStickY), stick(Axis::RightStickX, Axis::RightStickY));
        let done = BUTTONS.iter().filter(|b| self.tested.contains(b)).count();
        let style = self.style();

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

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Layout").size(13.0).color(theme::TEXT_DIM));
                let opts = [
                    (Choice::Auto, "Auto", "Follow the connected controller"),
                    (Choice::Xbox, "Xbox", "A B X Y, LB / RB, LT / RT"),
                    (Choice::PlayStation, "PlayStation", "Cross Circle Square Triangle, L1 / R1, L2 / R2"),
                    (Choice::Nintendo, "Nintendo", "B A Y X, L / R, ZL / ZR"),
                ];
                theme::segmented(ui, "gp-layout", &opts, &mut self.choice);
                ui.add_space(8.0);
                let note = if self.choice == Choice::Auto {
                    if connected { format!("Detected: {}", style.name()) } else { "Detects your controller when you plug it in".into() }
                } else {
                    format!("Showing: {}", style.name())
                };
                ui.label(egui::RichText::new(note).size(12.5).color(theme::TEXT_DIM));
            });

            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 370.0), Sense::hover());
            self.draw_pad(ui, area, style, &pressed, lt, rt, ls, rs, connected, now);

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("{done} / {} buttons", BUTTONS.len())).font(theme::bold(13.0)).color(Color32::WHITE));
                ui.add_space(6.0);
                for b in BUTTONS {
                    chip(ui, label(style, b), self.tested.contains(&b));
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
                    "The layout follows your controller; pick another above if a third-party pad looks wrong. \
                     Let go of both sticks to check for drift. The Guide / PS / Home button may be grabbed by Steam or the Xbox Game Bar.",
                )
                .size(12.0)
                .color(theme::TEXT_DIM),
            );
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_pad(&self, ui: &egui::Ui, area: Rect, style: Style, pressed: &HashSet<Button>, lt: f32, rt: f32, ls: Vec2, rs: Vec2, connected: bool, now: f64) {
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
            p.text(c + Vec2::new(0.0, 135.0 * s), Align2::CENTER_CENTER, "No controller", theme::bold(18.0), alpha(theme::TEXT_DIM, 0.9));
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
            txt
        };
        let name = |b: Button| label(style, b);

        // Where things go. PlayStation pads have the D-pad up top and both sticks low and
        // side by side; Xbox and Switch pads have the left stick up top and the D-pad low.
        let (left_stick, dp, right_stick) = if style.is_ps() {
            (c + Vec2::new(-80.0 * s, 50.0 * s), c + Vec2::new(-165.0 * s, -30.0 * s), c + Vec2::new(80.0 * s, 50.0 * s))
        } else {
            (c + Vec2::new(-165.0 * s, -30.0 * s), c + Vec2::new(-80.0 * s, 50.0 * s), c + Vec2::new(80.0 * s, 50.0 * s))
        };

        // Face buttons (right) in a diamond.
        let face = c + Vec2::new(165.0 * s, -30.0 * s);
        let fb = 34.0 * s;
        for (b, off) in [
            (Button::North, Vec2::new(0.0, -38.0)),
            (Button::South, Vec2::new(0.0, 38.0)),
            (Button::West, Vec2::new(-38.0, 0.0)),
            (Button::East, Vec2::new(38.0, 0.0)),
        ] {
            let rect = Rect::from_center_size(face + off * s, Vec2::splat(fb));
            if style.is_ps() {
                let col = part(rect, fb / 2.0, b, "");
                ps_symbol(p, rect.center(), fb, b, col);
            } else {
                part(rect, fb / 2.0, b, name(b));
            }
        }
        // D-pad.
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
        let small = |x: f32, y: f32, w: f32, b: Button| {
            part(Rect::from_center_size(c + Vec2::new(x * s, y * s), Vec2::new(w * s, 20.0 * s)), 10.0 * s, b, name(b));
        };
        match style {
            Style::Ps4 | Style::Ps5 => {
                // Touchpad (drawn for the look; it isn't testable here).
                let pad = Rect::from_center_size(c + Vec2::new(0.0, -58.0 * s), Vec2::new(150.0 * s, 66.0 * s));
                theme::vgradient(p, pad, 10.0 * s, Color32::from_rgb(40, 40, 44), Color32::from_rgb(26, 26, 29));
                p.rect_stroke(pad, 10.0 * s, Stroke::new(1.0, alpha(Color32::WHITE, 0.15)), StrokeKind::Inside);
                p.text(pad.center(), Align2::CENTER_CENTER, "touchpad", egui::FontId::proportional((11.0 * s).max(9.0)), alpha(theme::TEXT_DIM, 0.7));
                small(-112.0, -74.0, 56.0, Button::Select);
                small(112.0, -74.0, 62.0, Button::Start);
                part(Rect::from_center_size(c + Vec2::new(0.0, 14.0 * s), Vec2::splat(30.0 * s)), 15.0 * s, Button::Mode, "PS");
            }
            Style::Ps3 => {
                small(-48.0, -45.0, 56.0, Button::Select);
                small(48.0, -45.0, 50.0, Button::Start);
                part(Rect::from_center_size(c + Vec2::new(0.0, -6.0 * s), Vec2::splat(30.0 * s)), 15.0 * s, Button::Mode, "PS");
            }
            Style::Nintendo => {
                small(-45.0, -62.0, 26.0, Button::Select);
                small(45.0, -62.0, 26.0, Button::Start);
                part(Rect::from_center_size(c + Vec2::new(30.0 * s, -18.0 * s), Vec2::splat(26.0 * s)), 13.0 * s, Button::Mode, "⌂");
            }
            Style::Xbox => {
                small(-45.0, -45.0, 40.0, Button::Select);
                small(45.0, -45.0, 40.0, Button::Start);
                part(Rect::from_center_size(c + Vec2::new(0.0, -78.0 * s), Vec2::splat(30.0 * s)), 15.0 * s, Button::Mode, "⌂");
            }
        }
        // Bumpers and trigger bars on top.
        for (side, bump, trig, value) in [(-1.0f32, Button::LeftTrigger, Button::LeftTrigger2, lt), (1.0, Button::RightTrigger, Button::RightTrigger2, rt)] {
            let (bl, tl) = (name(bump), name(trig));
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
            (left_stick, ls, &self.left, Button::LeftThumb),
            (right_stick, rs, &self.right, Button::RightThumb),
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
    fn detects_controller_families() {
        assert_eq!(detect(Some(0x054C), Some(0x0268), "Sony PLAYSTATION(R)3 Controller"), Style::Ps3);
        assert_eq!(detect(Some(0x054C), Some(0x09CC), "Wireless Controller"), Style::Ps4);
        assert_eq!(detect(Some(0x054C), Some(0x0CE6), "DualSense Wireless Controller"), Style::Ps5);
        assert_eq!(detect(None, None, "PS3 Controller"), Style::Ps3);
        assert_eq!(detect(Some(0x045E), Some(0x0B12), "Xbox Series X Controller"), Style::Xbox);
        assert_eq!(detect(Some(0x057E), Some(0x2009), "Pro Controller"), Style::Nintendo);
        assert_eq!(detect(Some(0x046D), Some(0xC21D), "Logitech Gamepad F310"), Style::Xbox);
    }

    #[test]
    fn every_button_has_a_label_in_every_style() {
        for st in [Style::Xbox, Style::Ps3, Style::Ps4, Style::Ps5, Style::Nintendo] {
            for b in BUTTONS {
                assert_ne!(label(st, b), "?", "{st:?} {b:?}");
            }
        }
        assert_eq!(label(Style::Ps3, Button::Select), "Select");
        assert_eq!(label(Style::Ps4, Button::Start), "Options");
    }

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
