//! Gamepad test: buttons, D-pad, triggers, sticks (with drift detection) and rumble.

use crate::i18n::t;
use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use gilrs::{Axis, Button, GamepadId, Gilrs};
use std::collections::{HashSet, VecDeque};

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

/// Swap the left and top face buttons when the pad reports them the wrong way round.
fn swap_xy(b: Button, swap: bool) -> Button {
    match (swap, b) {
        (true, Button::West) => Button::North,
        (true, Button::North) => Button::West,
        _ => b,
    }
}

/// Linux's Xbox driver (`xpad`) sends the left button (X) under the code that officially
/// means "top", and the top one (Y) under "left". gilrs fixes that from SDL's controller
/// list, but only for the exact models and firmware versions listed there: an Xbox 360 pad
/// with firmware 0x0106, for one, isn't, so X lit up Y. When gilrs falls back to the
/// driver's codes for an xpad pad, swap them back.
fn needs_xy_swap(pad: &gilrs::Gamepad) -> bool {
    cfg!(target_os = "linux")
        && pad.mapping_source() == gilrs::MappingSource::Driver
        && pad.vendor_id().zip(pad.product_id()).is_some_and(|(v, p)| linux_driver(v, p).as_deref() == Some("xpad"))
}

/// The kernel driver behind a game controller with these USB IDs (Linux only).
fn linux_driver(vendor: u16, product: u16) -> Option<String> {
    let dir = std::fs::read_dir("/sys/class/input").ok()?;
    dir.flatten().find_map(|e| {
        let id = |f: &str| std::fs::read_to_string(e.path().join("device/id").join(f)).ok().and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
        if id("vendor")? != vendor || id("product")? != product {
            return None;
        }
        let driver = std::fs::read_link(e.path().join("device/device/driver")).ok()?;
        Some(driver.file_name()?.to_string_lossy().into_owned())
    })
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
        (B::Mode, Style::Nintendo) => "HOME",
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

/// Rest offsets: under SLIGHT a stick centres well, from DRIFT up games may move on their own
/// (many games ignore the middle 10–20%, so slight drift is usually hidden).
pub const SLIGHT: f32 = 0.05;
pub const DRIFT: f32 = 0.10;
/// How long the "let go of both sticks" measurement runs.
const MEASURE_SECS: f64 = 3.0;
/// The drift plot shows the middle quarter of the stick's travel.
const ZOOM: f32 = 0.25;
/// Live samples kept for the plot (about 1.5 s).
const TRAIL: usize = 90;

/// Where a stick rested during the measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rest {
    /// Average position (0 = dead centre, 1 = the edge).
    pub offset: Vec2,
    /// Furthest any sample was from the average: the stick's jitter.
    pub spread: f32,
    /// The stick was moved during the test, so the result means nothing.
    pub moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Good,
    Slight,
    Drift,
    Moved,
}

impl Rest {
    pub fn level(&self) -> Level {
        let off = self.offset.length();
        if self.moved {
            Level::Moved
        } else if off >= DRIFT {
            Level::Drift
        } else if off >= SLIGHT {
            Level::Slight
        } else {
            Level::Good
        }
    }
}

/// Average position and jitter of a stick that should be resting. None if too few samples.
pub fn analyse(samples: &[Vec2]) -> Option<Rest> {
    if samples.len() < 10 {
        return None;
    }
    let mean = samples.iter().fold(Vec2::ZERO, |a, v| a + *v) / samples.len() as f32;
    let spread = samples.iter().map(|v| (*v - mean).length()).fold(0.0, f32::max);
    let moved = spread > 0.15 || samples.iter().any(|v| v.length() > 0.5);
    Some(Rest { offset: mean, spread, moved })
}

fn level_text(l: Level) -> &'static str {
    match l {
        Level::Good => "centres well",
        Level::Slight => "slight drift, most games hide it",
        Level::Drift => "drift, games may move on their own",
        Level::Moved => "moved during the test, try again",
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
    /// Recent raw stick positions (left, right) for the drift plot.
    trail: [VecDeque<Vec2>; 2],
    /// The drift measurement: when it started, and the samples so far.
    measuring: Option<f64>,
    samples: [Vec<Vec2>; 2],
    pub measured: Option<[Rest; 2]>,
    /// Each stick's dead zone as the controller reports it, if it does.
    deadzones: [Option<f32>; 2],
    /// Which rumble was tried last, and the user's answer to "did you feel it?".
    rumble_kind: Option<Motor>,
    pub rumble_felt: Option<bool>,
    /// 6-axis motion sensors (PlayStation 4 / 5 controllers).
    pub motion: crate::motion::Motion,
    /// The pad reports X and Y the wrong way round (see `needs_xy_swap`).
    swap_xy: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Motor {
    Strong,
    Weak,
    Both,
}

impl Default for GamepadTest {
    fn default() -> Self {
        // Raw stick values: gilrs' default filters zero everything inside the controller's
        // dead zone, which would hide mild drift. The D-pad and jitter filters run in poll().
        let (gilrs, error) = match gilrs::GilrsBuilder::new().with_default_filters(false).set_update_state(false).build() {
            Ok(g) => (Some(g), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let mut s = Self {
            gilrs,
            error,
            active: None,
            tested: HashSet::new(),
            left: Default::default(),
            right: Default::default(),
            rumble_tried: false,
            effect: None,
            name: None,
            detected: Style::Xbox,
            choice: dev_choice(),
            trail: Default::default(),
            measuring: None,
            samples: Default::default(),
            measured: None,
            deadzones: [None, None],
            rumble_kind: None,
            rumble_felt: None,
            motion: Default::default(),
            swap_xy: false,
        };
        s.apply_demo();
        s
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
        use gilrs::ev::filter::{axis_dpad_to_button, Filter, Jitter};
        let jitter = Jitter::new();
        while let Some(raw) = g.next_event() {
            let Some(ev) = Some(raw).filter_ev(&axis_dpad_to_button, g).filter_ev(&jitter, g) else { continue };
            if ev.is_dropped() {
                continue;
            }
            g.update(&ev);
            any = true;
            // Follow whichever pad was used last.
            if self.active != Some(ev.id) {
                self.active = Some(ev.id);
                let pad = g.gamepad(ev.id);
                self.name = Some(pad.name().to_string());
                self.detected = detect(pad.vendor_id(), pad.product_id(), pad.name());
                self.swap_xy = needs_xy_swap(&pad);
            }
            if let gilrs::EventType::ButtonPressed(b, _) = ev.event {
                self.tested.insert(swap_xy(b, self.swap_xy));
            }
            if let gilrs::EventType::ButtonChanged(b, v, _) = ev.event {
                if v > 0.5 {
                    self.tested.insert(swap_xy(b, self.swap_xy));
                }
            }
        }
        if self.active.is_none() {
            if let Some((id, pad)) = g.gamepads().next() {
                self.active = Some(id);
                self.name = Some(pad.name().to_string());
                self.detected = detect(pad.vendor_id(), pad.product_id(), pad.name());
                self.swap_xy = needs_xy_swap(&pad);
            }
        }
        if let Some(pad) = self.active.and_then(|id| g.connected_gamepad(id)) {
            let l = Vec2::new(pad.value(Axis::LeftStickX), -pad.value(Axis::LeftStickY));
            let r = Vec2::new(pad.value(Axis::RightStickX), -pad.value(Axis::RightStickY));
            self.left.update(l, now);
            self.right.update(r, now);
            let dz = |x: Axis| pad.axis_code(x).and_then(|c| pad.deadzone(c));
            self.deadzones = [dz(Axis::LeftStickX), dz(Axis::RightStickX)];
            for (i, v) in [l, r].into_iter().enumerate() {
                self.trail[i].push_back(v);
                while self.trail[i].len() > TRAIL {
                    self.trail[i].pop_front();
                }
            }
            if let Some(start) = self.measuring {
                self.samples[0].push(l);
                self.samples[1].push(r);
                if now - start >= MEASURE_SECS {
                    self.finish_measuring();
                }
            }
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

    /// A controller is plugged in right now.
    pub fn connected(&self) -> bool {
        self.gilrs.as_ref().is_some_and(|g| g.gamepads().next().is_some())
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
        let measured_drift = self.measured.is_some_and(|m| m.iter().any(|r| r.level() == Level::Drift));
        let drift = self.left.drift.is_some() || self.right.drift.is_some() || measured_drift;
        let m = &self.motion.checks;
        let status = if drift || self.rumble_felt == Some(false) || m.gravity_wrong() || m.gyro_drifts() {
            Status::Issue
        } else if self.name.is_none()
            || (done == 0 && self.left.max_reach < 0.05 && self.right.max_reach < 0.05 && self.measured.is_none() && !m.tested())
        {
            Status::NotTested
        } else if done == BUTTONS.len() && self.left_ok() && self.right_ok() {
            Status::Pass
        } else {
            Status::InProgress
        };
        let mut lines = Vec::new();
        if let Some(n) = &self.name {
            lines.push(tr!("Controller: {0} ({1} layout)", n, style.name()));
        }
        lines.push(tr!("{0} / {1} buttons work", done, BUTTONS.len()));
        if done > 0 && done < BUTTONS.len() {
            let missing: Vec<&str> = BUTTONS.iter().filter(|b| !self.tested.contains(b)).map(|b| t(label(style, *b))).collect();
            lines.push(tr!("Not pressed yet: {0}", missing.join(", ")));
        }
        for (left, w) in [(true, &self.left), (false, &self.right)] {
            let range = t(if w.max_reach > 0.9 {
                "full range"
            } else if w.max_reach > 0.05 {
                "not pushed to the edge yet"
            } else {
                "not tested"
            });
            let drift = w.drift.map_or(String::new(), |v| tr!(", DRIFT: rests {0}% off-centre", format!("{:.0}", v.length() * 100.0)));
            lines.push(if left { tr!("Left stick: {0}{1}", range, drift) } else { tr!("Right stick: {0}{1}", range, drift) });
        }
        if let Some(m) = self.measured {
            for (left, r) in [(true, m[0]), (false, m[1])] {
                let pct = format!("{:.1}", r.offset.length() * 100.0);
                let verdict = t(level_text(r.level()));
                lines.push(if left {
                    tr!("Left stick at rest: {0}% off-centre, {1}", pct, verdict)
                } else {
                    tr!("Right stick at rest: {0}% off-centre, {1}", pct, verdict)
                });
            }
        }
        if m.tested() {
            lines.push(tr!("Motion: tilted {0} of 3 ways, turned {1} of 3 ways", m.tilts_done(), m.turns_done()));
            if let Some(g) = m.rest_gravity {
                lines.push(tr!("Accelerometer at rest: {0} g (should be about 1 g)", format!("{g:.2}")));
            }
            if let Some(d) = m.rest_gyro {
                lines.push(tr!("Gyroscope at rest: {0} °/s (should be near 0)", format!("{d:.1}")));
            }
        }
        match self.rumble_felt {
            Some(true) => lines.push(t("Vibration: works").into()),
            Some(false) => lines.push(t("Vibration: not felt").into()),
            None if self.rumble_tried => lines.push(t("Rumble: tried").into()),
            None => {}
        }
        Section { title: "Gamepad", jp: "ゲームパッド", status, lines }
    }

    /// Stop any vibration that's still playing.
    pub fn stop_rumble(&mut self) {
        if let Some(e) = self.effect.take() {
            let _ = e.stop();
        }
    }

    pub fn reset(&mut self) {
        self.stop_rumble();
        self.motion.reset();
        self.tested.clear();
        self.left = Default::default();
        self.right = Default::default();
        self.rumble_tried = false;
        self.rumble_kind = None;
        self.rumble_felt = None;
        self.measuring = None;
        self.measured = None;
        self.samples = Default::default();
    }

    fn start_measuring(&mut self, now: f64) {
        self.measuring = Some(now);
        self.samples = Default::default();
    }

    fn finish_measuring(&mut self) {
        self.measuring = None;
        if let (Some(l), Some(r)) = (analyse(&self.samples[0]), analyse(&self.samples[1])) {
            self.measured = Some([l, r]);
        }
    }

    /// Dev: NYANSPECT_DRIFT_DEMO=1 fills in a measurement (left centres well, right drifts).
    fn apply_demo(&mut self) {
        if std::env::var("NYANSPECT_DRIFT_DEMO").is_err() {
            return;
        }
        let mut seed = 7u32;
        let mut noise = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as f32 / 65_536.0 - 0.5
        };
        for i in 0..TRAIL {
            self.trail[0].push_back(Vec2::new(0.021 + noise() * 0.02, -0.013 + noise() * 0.02));
            self.trail[1].push_back(Vec2::new(0.128 + noise() * 0.03, 0.061 + noise() * 0.03));
            if i % 2 == 0 {
                let (l, r) = (*self.trail[0].back().unwrap(), *self.trail[1].back().unwrap());
                self.samples[0].push(l);
                self.samples[1].push(r);
            }
        }
        self.finish_measuring();
        self.deadzones = [Some(0.08), Some(0.08)];
        self.name = Some("Demo Controller".into());
        self.rumble_kind = Some(Motor::Both);
    }

    fn rumble(&mut self, motor: Motor) {
        let (Some(g), Some(id)) = (self.gilrs.as_mut(), self.active) else { return };
        use gilrs::ff::{BaseEffect, BaseEffectType, EffectBuilder, Replay, Ticks};
        // Stop anything still playing first.
        if let Some(old) = self.effect.take() {
            let _ = old.stop();
        }
        let mut builder = EffectBuilder::new();
        // gilrs repeats an effect forever unless told otherwise: play it once.
        builder.repeat(gilrs::ff::Repeat::For(Ticks::from_ms(700)));
        if motor != Motor::Weak {
            builder.add_effect(BaseEffect {
                kind: BaseEffectType::Strong { magnitude: 45_000 },
                scheduling: Replay { play_for: Ticks::from_ms(700), ..Default::default() },
                ..Default::default()
            });
        }
        if motor != Motor::Strong {
            builder.add_effect(BaseEffect {
                kind: BaseEffectType::Weak { magnitude: 50_000 },
                scheduling: Replay { play_for: Ticks::from_ms(700), ..Default::default() },
                ..Default::default()
            });
        }
        if let Ok(e) = builder.gamepads(&[id]).finish(g) {
            let _ = e.play();
            self.effect = Some(e);
            self.rumble_tried = true;
            self.rumble_kind = Some(motor);
            self.rumble_felt = None;
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        let now = ui.input(|i| i.time);
        let pad = self.gilrs.as_ref().and_then(|g| self.active.and_then(|id| g.connected_gamepad(id)));
        let connected = pad.is_some();
        if connected {
            // Controllers are polled once per frame: keep frames coming so sticks feel live.
            ui.ctx().request_repaint();
        }
        let ff = pad.as_ref().is_some_and(|p| p.is_ff_supported());
        let pressed: HashSet<Button> = pad
            .as_ref()
            .map(|p| BUTTONS.iter().copied().filter(|b| p.is_pressed(swap_xy(*b, self.swap_xy))).collect())
            .unwrap_or_default();
        let trig = |b: Button| pad.as_ref().and_then(|p| p.button_data(b)).map_or(0.0, |d| d.value());
        let (lt, rt) = (trig(Button::LeftTrigger2), trig(Button::RightTrigger2));
        let stick = |x: Axis, y: Axis| pad.as_ref().map_or(Vec2::ZERO, |p| Vec2::new(p.value(x), -p.value(y)));
        let (ls, rs) = (stick(Axis::LeftStickX, Axis::LeftStickY), stick(Axis::RightStickX, Axis::RightStickY));
        let done = BUTTONS.iter().filter(|b| self.tested.contains(b)).count();
        let style = self.style();

        let measured_drift = self.measured.is_some_and(|m| m.iter().any(|r| r.level() == Level::Drift));
        if self.measuring.is_some() {
            mascot.say(t("Hands off the sticks… measuring~"));
        } else if self.left.drift.is_some() || self.right.drift.is_some() || measured_drift {
            mascot.say(t("A stick isn't centring… stick drift! (；ω；)"));
        } else if !connected {
            mascot.say(t("Plug in or pair a controller, then press a button~"));
        } else if done == BUTTONS.len() && self.left_ok() && self.right_ok() {
            mascot.say(t("Your controller is perfect! ＼(^o^)／"));
        } else {
            mascot.say(t("Press every button and roll both sticks around~ ♪"));
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::fancy_button(ui, t("↺ Reset"), Vec2::new(100.0, 32.0), theme::SILVER, theme::STEEL, true).clicked() {
                        self.reset();
                    }
                    ui.add_space(10.0);
                    let label = match (&self.error, &self.name, connected) {
                        (Some(e), _, _) => tr!("Controllers unavailable: {0}", e),
                        (_, Some(n), true) => n.clone(),
                        _ => t("No controller connected").to_string(),
                    };
                    ui.label(egui::RichText::new(label).size(12.5).color(theme::TEXT_DIM));
                });
            });

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(t("Layout")).size(13.0).color(theme::TEXT_DIM));
                let opts = [
                    (Choice::Auto, t("Auto"), t("Follow the connected controller")),
                    (Choice::Xbox, "Xbox", "A B X Y, LB / RB, LT / RT"),
                    (Choice::PlayStation, "PlayStation", "Cross Circle Square Triangle, L1 / R1, L2 / R2"),
                    (Choice::Nintendo, "Nintendo", "B A Y X, L / R, ZL / ZR"),
                ];
                theme::segmented(ui, "gp-layout", &opts, &mut self.choice);
                ui.add_space(8.0);
                let note = if self.choice == Choice::Auto {
                    if connected { tr!("Detected: {0}", style.name()) } else { t("Detects your controller when you plug it in").into() }
                } else {
                    tr!("Showing: {0}", style.name())
                };
                ui.label(egui::RichText::new(note).size(12.5).color(theme::TEXT_DIM));
            });

            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 370.0), Sense::hover());
            self.draw_pad(ui, area, style, &pressed, lt, rt, ls, rs, connected, now);

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(tr!("{0} / {1} buttons", done, BUTTONS.len())).font(theme::bold(13.0)).color(Color32::WHITE));
                ui.add_space(6.0);
                for b in BUTTONS {
                    chip(ui, t(label(style, b)), self.tested.contains(&b));
                }
                chip(ui, t("Left stick full range"), self.left_ok());
                chip(ui, t("Right stick full range"), self.right_ok());
            });
            for (left, d) in [(true, self.left.drift), (false, self.right.drift)] {
                if let Some(v) = d {
                    let pct = format!("{:.0}", v.length() * 100.0);
                    let msg = if left {
                        tr!("⚠ Left stick drift: it rests at {0}% off-centre instead of the middle. Games may move on their own.", pct)
                    } else {
                        tr!("⚠ Right stick drift: it rests at {0}% off-centre instead of the middle. Games may move on their own.", pct)
                    };
                    ui.label(
                        egui::RichText::new(msg)
                        .font(theme::bold(13.0))
                        .color(Color32::WHITE),
                    );
                }
            }
            ui.label(
                egui::RichText::new(t(
                    "The layout follows your controller; pick another above if a third-party pad looks wrong. Let go of both sticks to check for drift. The Guide / PS / Home button may be grabbed by Steam or the Xbox Game Bar.",
                )
                )
                .size(12.0)
                .color(theme::TEXT_DIM),
            );
        });
        ui.add_space(12.0);
        let demo = std::env::var("NYANSPECT_DRIFT_DEMO").is_ok();
        self.drift_card(ui, cjk, connected || demo, connected.then_some((ls, rs)), now);
        ui.add_space(12.0);
        self.motion_card(ui, cjk);
        ui.add_space(12.0);
        self.vibration_card(ui, cjk, ff, demo);
    }

    fn drift_card(&mut self, ui: &mut egui::Ui, cjk: bool, available: bool, live: Option<(Vec2, Vec2)>, now: f64) {
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            theme::card_title(ui, "Stick drift test", "スティックのドリフト", cjk);
            let plot = ((ui.available_width() - 420.0) / 2.0).clamp(150.0, 230.0);
            ui.horizontal_top(|ui| {
                for (i, (name, live)) in [(t("Left stick"), live.map(|l| l.0)), (t("Right stick"), live.map(|l| l.1))].into_iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_width(plot);
                        ui.label(egui::RichText::new(name).font(theme::bold(13.5)).color(Color32::WHITE));
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(plot), Sense::hover());
                        let rest = self.measured.map(|m| m[i]);
                        draw_rest_plot(ui.painter(), rect, &self.trail[i], rest, self.deadzones[i], live);
                        if let Some(r) = rest {
                            let level = r.level();
                            let text = tr!("{0}% off-centre", format!("{:.1}", r.offset.length() * 100.0));
                            ui.label(egui::RichText::new(text).font(theme::bold(15.0)).color(Color32::WHITE));
                            let col = if matches!(level, Level::Drift) { theme::GLINT } else { theme::TEXT };
                            let mark = match level {
                                Level::Good => "✔ ",
                                Level::Drift | Level::Moved => "⚠ ",
                                Level::Slight => "",
                            };
                            ui.label(egui::RichText::new(format!("{mark}{}", t(level_text(level)))).size(13.0).color(col));
                            ui.label(egui::RichText::new(tr!("Jitter: {0}%", format!("{:.1}", r.spread * 100.0))).size(12.0).color(theme::TEXT_DIM));
                        }
                    });
                    ui.add_space(18.0);
                }
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(t(
                            "Put the controller down and let go of both sticks, then press Measure. Nyanspect reads where each stick rests for 3 seconds.",
                        ))
                        .size(13.5)
                        .color(theme::TEXT),
                    );
                    ui.add_space(8.0);
                    match self.measuring {
                        Some(start) => {
                            let left = (MEASURE_SECS - (now - start)).max(0.0);
                            let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(260.0), 10.0), Sense::hover());
                            let p = ui.painter();
                            p.rect_filled(bar, 5.0, Color32::from_rgb(34, 34, 38));
                            let frac = (1.0 - left / MEASURE_SECS) as f32;
                            theme::hgradient(p, Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height())), 5.0, theme::STEEL, Color32::WHITE);
                            ui.label(egui::RichText::new(tr!("Hands off… {0} s", format!("{:.0}", left.ceil()))).font(theme::bold(14.0)).color(Color32::WHITE));
                            ui.ctx().request_repaint();
                        }
                        None => {
                            let label = if self.measured.is_some() { t("↻ Measure again") } else { t("▶ Measure drift") };
                            if theme::fancy_button(ui, label, Vec2::new(180.0, 38.0), theme::ACCENT, theme::SILVER, available && self.active.is_some()).clicked() {
                                self.start_measuring(now);
                            }
                            if !available {
                                ui.label(egui::RichText::new(t("Connect a controller first.")).size(12.5).color(theme::TEXT_DIM));
                            }
                        }
                    }
                    if self.measured.is_some_and(|m| m.iter().any(|r| r.moved)) {
                        ui.label(
                            egui::RichText::new(t("A stick moved during the test. Let go of both sticks completely and try again."))
                                .font(theme::bold(13.0))
                                .color(Color32::WHITE),
                        );
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(t(
                            "Under 5% off-centre is normal. 5–10% is slight drift that most games hide with their own dead zone. Over 10%, characters or cameras may move on their own.",
                        ))
                        .size(12.0)
                        .color(theme::TEXT_DIM),
                    );
                });
            });
        });
    }

    fn motion_card(&mut self, ui: &mut egui::Ui, cjk: bool) {
        use crate::motion::{Source, TILT_G, TURN_DPS};
        self.motion.start();
        self.motion.update();
        let source = self.motion.source();
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::card_title(ui, "Motion sensors (6-axis)", "モーションセンサー", cjk);
                if let Source::Live(name) = &source {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(name).size(12.5).color(theme::TEXT_DIM));
                    });
                }
            });
            let sample = match &source {
                Source::Live(_) => self.motion.last,
                _ => None,
            };
            let Some(s) = sample else {
                let msg = match &source {
                    Source::NeedsPermission(name) => tr!(
                        "Found {0}, but Linux doesn't allow reading its motion sensors. Install your distribution's game controller rules (for example steam-devices or game-devices-udev), then plug the controller in again.",
                        name
                    ),
                    Source::Live(_) => t("Waiting for the controller's motion data… move it a little.").to_string(),
                    Source::Searching => t(
                        "Connect a PlayStation 4 or 5 controller (DualShock 4 / DualSense) to test its accelerometer and gyroscope. Xbox controllers don't have motion sensors.",
                    )
                    .to_string(),
                };
                theme::note(ui, &msg, 13.0);
                return;
            };
            ui.ctx().request_repaint();
            ui.horizontal_top(|ui| {
                // Bubble level: X and Z acceleration move the bubble; tilt to roll it around.
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(150.0), Sense::hover());
                let p = ui.painter();
                let c = rect.center();
                let r = rect.width() / 2.0 - 4.0;
                p.circle_filled(c, r, Color32::from_rgba_unmultiplied(8, 8, 10, 230));
                p.circle_stroke(c, r, Stroke::new(1.0, alpha(theme::LIGHT, 0.25)));
                p.circle_stroke(c, r * 0.5, Stroke::new(1.0, alpha(theme::LIGHT, 0.12)));
                p.line_segment([c - Vec2::new(r, 0.0), c + Vec2::new(r, 0.0)], Stroke::new(1.0, alpha(theme::LIGHT, 0.08)));
                p.line_segment([c - Vec2::new(0.0, r), c + Vec2::new(0.0, r)], Stroke::new(1.0, alpha(theme::LIGHT, 0.08)));
                let v = Vec2::new(s.accel[0], s.accel[2]);
                let v = if v.length() > 1.0 { v.normalized() } else { v };
                let bubble = c + v * (r - 10.0);
                theme::glow(p, bubble, 20.0, alpha(Color32::WHITE, 0.3));
                p.circle_filled(bubble, 9.0, Color32::WHITE);
                ui.add_space(18.0);

                // Live readings.
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() * 0.5).clamp(220.0, 380.0));
                    ui.label(egui::RichText::new(t("Accelerometer (g)")).font(theme::bold(13.5)).color(Color32::WHITE));
                    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                        meter(ui, axis, s.accel[i], 2.0, format!("{:+.2}", s.accel[i]));
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(t("Gyroscope (°/s)")).font(theme::bold(13.5)).color(Color32::WHITE));
                    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                        meter(ui, axis, s.gyro[i], 500.0, format!("{:+.0}", s.gyro[i]));
                    }
                });
                ui.add_space(18.0);

                // What's been checked.
                ui.vertical(|ui| {
                    theme::note(ui, t("Tilt the controller every way, then give it a quick twist around each axis. Put it down for a moment to check the sensors at rest."), 13.0);
                    ui.add_space(6.0);
                    let m = self.motion.checks;
                    ui.horizontal_wrapped(|ui| {
                        for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                            chip(ui, &tr!("Tilt {0}", axis), m.tilt[i] >= TILT_G);
                        }
                        for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                            chip(ui, &tr!("Turn {0}", axis), m.turn[i] >= TURN_DPS);
                        }
                    });
                    ui.add_space(6.0);
                    if let Some(g) = m.rest_gravity {
                        let text = tr!("At rest: {0} g (should be about 1 g)", format!("{g:.2}"));
                        ui.label(egui::RichText::new(if m.gravity_wrong() { format!("⚠ {text}") } else { text }).size(12.5).color(Color32::WHITE));
                    }
                    if let Some(d) = m.rest_gyro {
                        let text = tr!("Gyro at rest: {0} °/s (should be near 0)", format!("{d:.1}"));
                        ui.label(egui::RichText::new(if m.gyro_drifts() { format!("⚠ {text}") } else { text }).size(12.5).color(Color32::WHITE));
                    }
                });
            });
        });
    }

    fn vibration_card(&mut self, ui: &mut egui::Ui, cjk: bool, ff: bool, demo: bool) {
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            theme::card_title(ui, "Vibration", "振動", cjk);
            let can = ff || demo;
            ui.horizontal(|ui| {
                for (motor, label, hint) in [
                    (Motor::Strong, t("〰 Strong motor"), t("Usually in the left grip: a deep rumble")),
                    (Motor::Weak, t("〰 Weak motor"), t("Usually in the right grip: a fine buzz")),
                    (Motor::Both, t("〰 Both"), t("Both motors at once")),
                ] {
                    if theme::fancy_button(ui, label, Vec2::new(150.0, 36.0), theme::LIGHT, theme::SILVER, ff).on_hover_text(hint).clicked() {
                        self.rumble(motor);
                    }
                    ui.add_space(6.0);
                }
            });
            if !can {
                ui.label(egui::RichText::new(t("This controller doesn't support vibration here")).size(12.5).color(theme::TEXT_DIM));
            }
            if self.rumble_kind.is_some() {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t("Did you feel it vibrate?")).font(theme::bold(14.0)).color(Color32::WHITE));
                    ui.add_space(8.0);
                    let yes = self.rumble_felt == Some(true);
                    let no = self.rumble_felt == Some(false);
                    if theme::fancy_button(ui, t("Yes"), Vec2::new(80.0, 32.0), if yes { theme::ACCENT } else { theme::SILVER }, theme::STEEL, true).clicked() {
                        self.rumble_felt = Some(true);
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("No"), Vec2::new(80.0, 32.0), if no { theme::ACCENT } else { theme::SILVER }, theme::STEEL, true).clicked() {
                        self.rumble_felt = Some(false);
                    }
                    ui.add_space(10.0);
                    match self.rumble_felt {
                        Some(true) => {
                            ui.label(egui::RichText::new(t("✔ Vibration works")).font(theme::bold(13.5)).color(Color32::WHITE));
                        }
                        Some(false) => {
                            ui.label(egui::RichText::new(t("⚠ A motor may be broken or disconnected")).font(theme::bold(13.5)).color(theme::GLINT));
                        }
                        None => {}
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(t("Try each motor on its own: if one never vibrates, it's broken or unplugged inside the controller."))
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
            p.text(c + Vec2::new(0.0, 135.0 * s), Align2::CENTER_CENTER, t("No controller"), theme::bold(18.0), alpha(theme::TEXT_DIM, 0.9));
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
                p.text(pad.center(), Align2::CENTER_CENTER, t("touchpad"), egui::FontId::proportional((11.0 * s).max(9.0)), alpha(theme::TEXT_DIM, 0.7));
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
                p.text(centre + Vec2::new(0.0, r + 14.0), Align2::CENTER_CENTER, t("⚠ drift"), theme::bold(12.0), Color32::WHITE);
            }
        }
    }
}

/// A zoomed view of the middle of a stick's travel: rings at 5, 10 and 20%, the controller's
/// own dead zone (dotted), recent positions as fading dots, the live position, and the
/// measured rest position with a circle as big as its jitter.
fn draw_rest_plot(p: &egui::Painter, rect: Rect, trail: &VecDeque<Vec2>, rest: Option<Rest>, deadzone: Option<f32>, live: Option<Vec2>) {
    let c = rect.center();
    let r = rect.width() / 2.0 - 4.0;
    let at = |v: Vec2| {
        let z = v / ZOOM;
        c + if z.length() > 1.0 { z.normalized() } else { z } * r
    };
    p.circle_filled(c, r, Color32::from_rgba_unmultiplied(8, 8, 10, 230));
    p.circle_stroke(c, r, Stroke::new(1.0, alpha(theme::LIGHT, 0.25)));
    p.line_segment([c - Vec2::new(r, 0.0), c + Vec2::new(r, 0.0)], Stroke::new(1.0, alpha(theme::LIGHT, 0.08)));
    p.line_segment([c - Vec2::new(0.0, r), c + Vec2::new(0.0, r)], Stroke::new(1.0, alpha(theme::LIGHT, 0.08)));
    for (ring, label) in [(SLIGHT, "5%"), (DRIFT, "10%"), (0.20, "20%")] {
        let rr = r * ring / ZOOM;
        let strong = ring == DRIFT;
        p.circle_stroke(c, rr, Stroke::new(if strong { 1.5 } else { 1.0 }, alpha(theme::LIGHT, if strong { 0.45 } else { 0.18 })));
        p.text(c + Vec2::new(rr * 0.707 + 2.0, -rr * 0.707 - 2.0), Align2::LEFT_BOTTOM, label, egui::FontId::proportional(10.0), theme::TEXT_DIM);
    }
    if let Some(d) = deadzone.filter(|d| *d > 0.005 && *d < ZOOM) {
        let rr = r * d / ZOOM;
        for k in 0..48 {
            let a = k as f32 / 48.0 * std::f32::consts::TAU;
            p.circle_filled(c + Vec2::angled(a) * rr, 1.0, alpha(theme::SILVER, 0.6));
        }
    }
    let n = trail.len().max(1) as f32;
    for (i, v) in trail.iter().enumerate() {
        p.circle_filled(at(*v), 2.0, alpha(Color32::WHITE, 0.08 + 0.4 * (i as f32 + 1.0) / n));
    }
    if let Some(rest) = rest {
        let m = at(rest.offset);
        let ring = (r * rest.spread / ZOOM).max(5.0);
        let col = if rest.level() == Level::Drift { theme::GLINT } else { Color32::WHITE };
        p.circle_stroke(m, ring, Stroke::new(1.5, alpha(col, 0.8)));
        p.line_segment([m - Vec2::new(6.0, 0.0), m + Vec2::new(6.0, 0.0)], Stroke::new(2.0, col));
        p.line_segment([m - Vec2::new(0.0, 6.0), m + Vec2::new(0.0, 6.0)], Stroke::new(2.0, col));
    }
    if let Some(v) = live {
        let dot = at(v);
        if (v / ZOOM).length() > 1.0 {
            p.circle_stroke(dot, 5.0, Stroke::new(1.5, Color32::WHITE));
        } else {
            theme::glow(p, dot, 12.0, alpha(Color32::WHITE, 0.3));
            p.circle_filled(dot, 4.5, Color32::WHITE);
        }
    }
}

/// One axis: a centred bar (left of the middle = negative) with its value.
fn meter(ui: &mut egui::Ui, axis: &str, value: f32, full: f32, text: String) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(axis).font(theme::bold(12.5)).color(theme::TEXT_DIM));
        let (bar, _) = ui.allocate_exact_size(Vec2::new((ui.available_width() - 60.0).max(80.0), 10.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(bar, 5.0, Color32::from_rgb(34, 34, 38));
        let mid = bar.center().x;
        let x = mid + (value / full).clamp(-1.0, 1.0) * bar.width() / 2.0;
        let fill = Rect::from_x_y_ranges(mid.min(x)..=mid.max(x), bar.y_range());
        if fill.width() > 0.5 {
            p.rect_filled(fill, 5.0, Color32::WHITE);
        }
        p.line_segment([Pos2::new(mid, bar.top() - 2.0), Pos2::new(mid, bar.bottom() + 2.0)], Stroke::new(1.0, alpha(theme::LIGHT, 0.5)));
        ui.label(egui::RichText::new(text).monospace().size(12.0).color(Color32::WHITE));
    });
}

fn chip(ui: &mut egui::Ui, label: &str, ok: bool) {
    theme::chip(ui, label, ok, 12.0);
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
    fn swapping_x_and_y_only_touches_x_and_y() {
        assert_eq!(swap_xy(Button::West, true), Button::North);
        assert_eq!(swap_xy(Button::North, true), Button::West);
        assert_eq!(swap_xy(Button::South, true), Button::South);
        assert_eq!(swap_xy(Button::West, false), Button::West);
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
    fn rest_measurement_levels() {
        let still = |x: f32, y: f32| -> Vec<Vec2> { (0..60).map(|i| Vec2::new(x, y) + Vec2::splat((i % 3) as f32 * 0.002)).collect() };
        assert_eq!(analyse(&still(0.01, -0.02)).unwrap().level(), Level::Good);
        assert_eq!(analyse(&still(0.06, 0.0)).unwrap().level(), Level::Slight);
        assert_eq!(analyse(&still(0.09, 0.08)).unwrap().level(), Level::Drift);
        let mut moved = still(0.0, 0.0);
        moved.push(Vec2::new(0.9, 0.0));
        assert_eq!(analyse(&moved).unwrap().level(), Level::Moved);
        assert!(analyse(&still(0.0, 0.0)[..5]).is_none());
        let r = analyse(&still(0.02, 0.0)).unwrap();
        assert!(r.spread < 0.01);
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
