use crate::chatter::ChatterDetector;
use crate::layouts::{self, Layout, Size, Style, *};
use crate::sfx::Sfx;
use crate::theme::{self, alpha, lerp_color, Mascot};
use eframe::egui::{self, Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};


struct Spark {
    pos: Pos2,
    vel: Vec2,
    life: f32,
    max: f32,
    color: Color32,
    size: f32,
}

struct Ring {
    rect: Rect,
    age: f32,
}

pub struct KeyboardTest {
    size: Size,
    style: Style,
    layout: Layout,
    chatter: ChatterDetector,
    /// When each key last chattered (for the warning flash).
    chatter_at: HashMap<Key, f32>,
    down: HashSet<Key>,
    tested: HashSet<Key>,
    log: VecDeque<(String, f32)>,
    sparks: Vec<Spark>,
    rings: Vec<Ring>,
    pending: Vec<(Key, bool)>,
    rng: u32,
    celebrated: bool,
    sfx: Sfx,
    /// Height the page may fill (set by the app each frame), so the keyboard scales to fit.
    pub fit_height: f32,
    last_card_h: f32,
}

impl Default for KeyboardTest {
    fn default() -> Self {
        Self {
            size: Size::Full,
            style: Style::Ansi,
            layout: layouts::build(Size::Full, Style::Ansi),
            chatter: ChatterDetector::default(),
            chatter_at: HashMap::new(),
            down: HashSet::new(),
            tested: HashSet::new(),
            log: VecDeque::new(),
            sparks: vec![],
            rings: vec![],
            pending: vec![],
            rng: 0xC0FF_EE11,
            celebrated: false,
            sfx: Sfx::default(),
            fit_height: 0.0,
            last_card_h: 0.0,
        }
    }
}

fn pretty(key: Key) -> String {
    let np = [
        (NP0, "Num 0"), (NP1, "Num 1"), (NP2, "Num 2"), (NP3, "Num 3"), (NP4, "Num 4"),
        (NP5, "Num 5"), (NP6, "Num 6"), (NP7, "Num 7"), (NP8, "Num 8"), (NP9, "Num 9"),
        (NP_DOT, "Num ."), (NP_ADD, "Num +"), (NP_SUB, "Num −"), (NP_MUL, "Num *"),
        (NP_DIV, "Num /"), (NP_ENTER, "Num Enter"),
    ];
    let named = [
        (Key::ShiftLeft, "L Shift"), (Key::ShiftRight, "R Shift"), (Key::ControlLeft, "L Ctrl"),
        (Key::ControlRight, "R Ctrl"), (Key::AltLeft, "L Alt"), (Key::AltRight, "R Alt"),
        (Key::SuperLeft, "L Win"), (Key::SuperRight, "R Win"), (Key::CapsLock, "Caps Lock"),
        (Key::NumLock, "Num Lock"), (Key::PrintScreen, "PrtSc"), (Key::ScrollLock, "ScrLk"),
    ];
    np.iter()
        .chain(named.iter())
        .find(|(k, _)| *k == key)
        .map(|(_, s)| s.to_string())
        .unwrap_or_else(|| key.name().to_string())
}

impl KeyboardTest {
    pub fn load_settings(&mut self, s: &crate::settings::Settings) {
        self.size = s.kb_size;
        self.style = s.kb_style;
        self.sfx.enabled = s.sfx;
        self.layout = layouts::build(self.size, self.style);
    }

    pub fn store_settings(&self, s: &mut crate::settings::Settings) {
        s.kb_size = self.size;
        s.kb_style = self.style;
        s.sfx = self.sfx.enabled;
    }

    /// Dev/screenshots: NYANSPECT_LAYOUT picks the layout, e.g. "tkl-iso".
    pub fn apply_env_layout(&mut self) {
        if let Ok(v) = std::env::var("NYANSPECT_LAYOUT") {
            self.size = if v.contains("tkl") { Size::Tkl } else if v.contains("compact") { Size::Compact } else { Size::Full };
            self.style = if v.contains("iso") { Style::Iso } else { Style::Ansi };
            self.layout = layouts::build(self.size, self.style);
        }
    }

    /// Sound effects triggered since the last call (empty when SFX is off).
    pub fn take_sounds(&mut self) -> Vec<crate::sfx_audio::SfxSound> {
        std::mem::take(&mut self.sfx.sounds)
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng % 10_000) as f32 / 10_000.0
    }

    fn handle_input(&mut self, ctx: &egui::Context, mascot: &mut Mascot) {
        let now = ctx.input(|i| i.time as f32);
        let events: Vec<_> = ctx.input(|i| i.events.clone());
        for ev in events {
            match ev {
                egui::Event::Key { key, physical_key, pressed, repeat, .. } => {
                    let key = physical_key.unwrap_or(key);
                    if pressed {
                        self.down.insert(key);
                        let first = self.tested.insert(key);
                        if !repeat {
                            self.pending.push((key, first));
                            if self.chatter.press(key, now as f64) {
                                self.chatter_at.insert(key, now);
                                self.log.push_front((format!("⚠ {} chatter", pretty(key)), now));
                            } else {
                                self.log.push_front((pretty(key), now));
                            }
                            mascot.cheer();
                        }
                    } else {
                        self.down.remove(&key);
                    }
                    self.log.truncate(10);
                }
                // If the window loses focus, release everything so nothing looks stuck.
                egui::Event::WindowFocused(false) => self.down.clear(),
                _ => {}
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        // Nothing on this tab may hold keyboard focus: every key press is test input.
        ui.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        self.handle_input(ui.ctx(), mascot);
        let (now, dt) = ui.input(|i| (i.time as f32, i.stable_dt.min(0.05)));
        let total = self.layout.caps.len();
        let done = self.layout.caps.iter().filter(|c| self.tested.contains(&c.key)).count();
        let frac = done as f32 / total as f32;
        let worst = self.chatter.worst();

        match done {
            _ if !worst.is_empty() => {
                mascot.say(format!("Hmm… {} pressed itself twice (；ω；)", pretty(worst[0].0)))
            }
            0 => mascot.say("Press any key on your keyboard~ ♪"),
            d if d == total => mascot.say("All keys work! Sugoi~! ＼(^o^)／"),
            _ if frac > 0.75 => mascot.say("Almost there, ganbatte!"),
            _ if frac > 0.4 => mascot.say("Nice, keep going nya~"),
            _ if self.size == Size::Full => mascot.say("Yay! Try every key, even the numpad!"),
            _ => mascot.say("Yay! Try every key on your keyboard!"),
        }

        // Centre the card vertically in any space left over (measured from the previous frame).
        if self.fit_height > 0.0 && self.last_card_h > 0.0 {
            ui.add_space(((self.fit_height - self.last_card_h) / 2.0 - 4.0).max(0.0));
        }
        let card = theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Keyboard test", "キーボードテスト", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::fancy_button(ui, "↺ Reset", Vec2::new(100.0, 32.0), theme::SILVER, theme::STEEL, true).clicked() {
                        self.tested.clear();
                        self.log.clear();
                        self.chatter.clear();
                        self.chatter_at.clear();
                        self.celebrated = false;
                    }
                    ui.add_space(6.0);
                    let label = if self.sfx.enabled { "★ SFX: ON" } else { "SFX: OFF" };
                    let (c1, c2) = if self.sfx.enabled { (theme::ACCENT, theme::SILVER) } else { (theme::STEEL, Color32::from_rgb(60, 60, 64)) };
                    if theme::fancy_button(ui, label, Vec2::new(108.0, 32.0), c1, c2, true)
                        .on_hover_text("Anime sound effects (pop-ups and sounds) when you press keys")
                        .clicked()
                    {
                        self.sfx.enabled = !self.sfx.enabled;
                    }
                    ui.add_space(12.0);
                    legend(ui, Color32::WHITE, "Holding");
                    legend(ui, Color32::from_rgb(130, 130, 136), "Works");
                    legend(ui, Color32::from_rgb(44, 44, 48), "Not tested");
                });
            });

            // Layout picker: size and ANSI/ISO style.
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Layout").size(13.0).color(theme::TEXT_DIM));
                let sizes: Vec<_> = Size::ALL.iter().map(|s| (*s, s.label(), s.hint())).collect();
                let styles: Vec<_> = Style::ALL.iter().map(|s| (*s, s.label(), s.hint())).collect();
                let changed = theme::segmented(ui, "kb-size", &sizes, &mut self.size)
                    | {
                        ui.add_space(10.0);
                        theme::segmented(ui, "kb-style", &styles, &mut self.style)
                    };
                if changed {
                    self.layout = layouts::build(self.size, self.style);
                    self.celebrated = false;
                }
                if self.size == Size::Compact {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Fn can't be tested: it never reaches apps.").size(12.0).color(theme::TEXT_DIM));
                }
            });

            // Fit both the width and the height of the window (the card's other rows need ~214 px).
            let (lw, lh) = (self.layout.w, self.layout.h);
            let by_width = ui.available_width() / lw;
            // The chatter summary adds another row (~32 px) below the keyboard.
            let reserved = if worst.is_empty() { 214.0 } else { 246.0 };
            let by_height = if self.fit_height > 0.0 { (self.fit_height - reserved) / lh } else { f32::MAX };
            let unit = by_width.min(by_height).clamp(26.0, 96.0);
            // Keys may get up to 15% taller to use spare height on wide screens.
            let unit_y = by_height.min(unit * 1.15).max(unit);
            let size = Vec2::new(unit * lw, unit_y * lh);
            ui.add_space(6.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), size.y + 4.0), Sense::hover());
            let origin = Pos2::new(rect.center().x - size.x / 2.0, rect.top());
            self.draw_keys(ui, origin, Vec2::new(unit, unit_y), now, dt);

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // progress bar
                let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() * 0.55, 18.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(bar, 9.0, Color32::from_rgba_unmultiplied(10, 10, 12, 210));
                let shown = ui.ctx().animate_value_with_time(ui.scope_id().with("kbprog"), frac, 0.35);
                if shown > 0.0 {
                    let fill = Rect::from_min_size(bar.min, Vec2::new((bar.width() * shown).max(18.0), bar.height()));
                    theme::rect_glow(p, fill, 9.0, theme::ACCENT, 0.4);
                    theme::hgradient(p, fill, 9.0, theme::STEEL, theme::SILVER);
                    // shimmer
                    let sx = fill.left() + ((now * 0.6) % 1.0) * fill.width();
                    theme::glow(p, Pos2::new(sx, fill.center().y), 14.0, alpha(Color32::WHITE, 0.5));
                }
                p.text(bar.center(), Align2::CENTER_CENTER, format!("{done} / {total} keys"), theme::bold(12.5), Color32::WHITE);

                ui.add_space(12.0);
                // recent keys as fading chips
                for (name, t) in self.log.iter().take(6) {
                    let age = now - t;
                    let a = (1.0 - age / 6.0).clamp(0.35, 1.0);
                    egui::Frame::new()
                        .fill(alpha(theme::SILVER, 0.18 * a))
                        .stroke(Stroke::new(1.0, alpha(theme::ACCENT, 0.6 * a)))
                        .corner_radius(10)
                        .inner_margin(egui::Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(name).size(12.5).color(alpha(theme::TEXT, a)));
                        });
                }
            });
            ui.add_space(4.0);
            if !worst.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("⚠ Chatter (one press counted twice):").font(theme::bold(13.0)).color(Color32::WHITE));
                    for (key, n) in worst.iter().take(8) {
                        egui::Frame::new()
                            .fill(Color32::WHITE)
                            .corner_radius(10)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(format!("{} ×{n}", pretty(*key))).size(12.5).color(theme::INK));
                            });
                    }
                    ui.label(egui::RichText::new("Press it a few more times to confirm; cleaning or replacing the switch usually fixes it.").size(12.0).color(theme::TEXT_DIM));
                });
                ui.add_space(2.0);
            }
            ui.label(
                egui::RichText::new("Tip: Win, PrtSc and some F-keys may be grabbed by your desktop before the app sees them.")
                    .size(12.0)
                    .color(theme::TEXT_DIM),
            );
        });
        self.last_card_h = card.response.rect.height();

        if done == total && !self.celebrated {
            self.celebrated = true;
            let c = ui.ctx().content_rect().center();
            for _ in 0..160 {
                let a = self.rand() * std::f32::consts::TAU;
                let s = 150.0 + self.rand() * 450.0;
                let color = [theme::ACCENT, theme::GLOW, theme::GLINT, theme::SILVER][(self.rand() * 3.99) as usize];
                let (m, z) = (self.rand(), self.rand());
                self.sparks.push(Spark { pos: c, vel: Vec2::angled(a) * s, life: 0.0, max: 1.2 + m, color, size: 4.0 + z * 5.0 });
            }
        }
        self.draw_sparks(ui.ctx(), dt);
        self.sfx.paint(ui.ctx(), dt, cjk);
    }

    fn draw_keys(&mut self, ui: &mut egui::Ui, origin: Pos2, units: Vec2, now: f32, dt: f32) {
        let unit = units.x;
        let ctx = ui.ctx().clone();
        let painter = ui.painter().clone();
        let gap = (unit * 0.08).max(2.5);
        let mut spawn = Vec::new();

        for cap in &self.layout.caps {
            let rect = Rect::from_min_size(
                origin + Vec2::new(cap.x * units.x, cap.y * units.y),
                Vec2::new(cap.w * units.x, cap.h * units.y),
            )
            .shrink(gap);
            let held = self.down.contains(&cap.key);
            let tested = self.tested.contains(&cap.key);
            let id = egui::Id::unique(("cap", cap.label, cap.x as i32 * 100 + cap.y as i32 * 10));
            let hold = ctx.animate_bool_with_time(id.with("hold"), held, 0.10);
            let ok = ctx.animate_bool_with_time(id.with("ok"), tested, 0.45);

            if let Some((_, first)) = self.pending.iter().find(|(k, _)| *k == cap.key) {
                spawn.push((cap.key, rect, *first));
            }

            let r = (unit * 0.18).max(5.0);
            let press = hold * unit * 0.07;
            let body = rect.translate(Vec2::new(0.0, press));
            // key "side" (3D depth)
            painter.rect_filled(rect.translate(Vec2::new(0.0, unit * 0.08)), r, Color32::from_rgba_unmultiplied(0, 0, 0, 220));

            if hold > 0.01 {
                theme::rect_glow(&painter, body, r, theme::GLOW, hold);
            } else if ok > 0.01 {
                theme::rect_glow(&painter, body, r, theme::ACCENT, ok * 0.12);
            }

            let idle_top = Color32::from_rgb(50, 50, 54);
            let idle_bot = Color32::from_rgb(28, 28, 31);
            let ok_top = lerp_color(idle_top, Color32::from_rgb(138, 138, 144), ok);
            let ok_bot = lerp_color(idle_bot, Color32::from_rgb(88, 88, 94), ok);
            let top = lerp_color(ok_top, Color32::from_rgb(255, 255, 255), hold);
            let bot = lerp_color(ok_bot, Color32::from_rgb(205, 205, 210), hold);
            theme::vgradient(&painter, body, r, top, bot);
            // top gloss
            let gloss = Rect::from_min_size(body.min + Vec2::new(3.0, 2.0), Vec2::new(body.width() - 6.0, body.height() * 0.42));
            theme::vgradient(&painter, gloss, r * 0.8, alpha(Color32::WHITE, 0.16 + 0.1 * ok), alpha(Color32::WHITE, 0.0));
            painter.rect_stroke(body, r, Stroke::new(1.0, alpha(Color32::WHITE, 0.10 + 0.25 * ok.max(hold))), StrokeKind::Inside);

            // Chatter warning: diagonal hazard stripes, a "!N" badge, and a pulse right after it happens.
            let chatters = self.chatter.count(cap.key);
            if chatters > 0 {
                let clip = painter.with_clip_rect(body.shrink(1.0));
                let step = (unit * 0.22).max(6.0);
                let mut x = body.left() - body.height();
                while x < body.right() {
                    clip.line_segment(
                        [Pos2::new(x, body.bottom()), Pos2::new(x + body.height(), body.top())],
                        Stroke::new(step * 0.35, alpha(theme::INK, 0.55)),
                    );
                    x += step;
                }
                painter.rect_stroke(body, r, Stroke::new(2.5, Color32::WHITE), StrokeKind::Inside);
                let since = self.chatter_at.get(&cap.key).map_or(9.0, |t| now - t);
                if since < 1.0 {
                    let e = since * unit * 0.5;
                    painter.rect_stroke(body.expand(e), r + e, Stroke::new(3.0 * (1.0 - since), alpha(Color32::WHITE, 1.0 - since)), StrokeKind::Outside);
                }
                let badge = Pos2::new(body.right() - 2.0, body.top() + 2.0);
                let br = (unit * 0.17).max(7.0);
                painter.circle(badge, br, Color32::WHITE, Stroke::new(1.5, theme::INK));
                let txt = if chatters > 9 { "!".to_string() } else { format!("!{chatters}") };
                painter.text(badge, Align2::CENTER_CENTER, txt, theme::bold(br * 1.05), theme::INK);
            }

            let label_size = if cap.label.chars().count() > 3 { unit * 0.24 } else { unit * 0.32 };
            let txt = lerp_color(lerp_color(theme::TEXT_DIM, Color32::WHITE, ok), theme::INK, hold);
            // Japanese fonts draw "\" as a yen sign, so use the built-in monospace font for it.
            let font = if cap.label == "\\" {
                FontId::monospace(label_size.max(9.5) * 1.1)
            } else {
                FontId::proportional(label_size.max(9.5))
            };
            painter.text(body.center(), Align2::CENTER_CENTER, cap.label, font, txt);
        }

        // Expanding rings for first-time presses.
        for &(key, rect, _) in &spawn {
            self.sfx.spawn(key, rect);
        }
        for (_, rect, first) in spawn {
            let n = if first { 14 } else { 6 };
            for _ in 0..n {
                let a = self.rand() * std::f32::consts::TAU;
                let s = 60.0 + self.rand() * if first { 220.0 } else { 120.0 };
                let color = [theme::ACCENT, theme::GLOW, theme::GLINT, theme::LIGHT][(self.rand() * 3.99) as usize];
                let (m, z) = (self.rand(), self.rand());
                self.sparks.push(Spark { pos: rect.center(), vel: Vec2::angled(a) * s, life: 0.0, max: 0.5 + m * 0.4, color, size: 3.0 + z * 3.5 });
            }
            if first {
                self.rings.push(Ring { rect, age: 0.0 });
            }
        }
        self.pending.clear();

        self.rings.retain_mut(|ring| {
            ring.age += dt;
            let t = ring.age / 0.5;
            let e = t * unit * 0.6;
            painter.rect_stroke(ring.rect.expand(e), unit * 0.2 + e, Stroke::new(2.5 * (1.0 - t), alpha(theme::ACCENT, 1.0 - t)), StrokeKind::Outside);
            t < 1.0
        });
        let _ = now;
    }

    fn draw_sparks(&mut self, ctx: &egui::Context, dt: f32) {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("sparks")));
        self.sparks.retain_mut(|s| {
            s.life += dt;
            s.vel *= 1.0 - 2.5 * dt;
            s.vel.y += 160.0 * dt;
            s.pos += s.vel * dt;
            let t = s.life / s.max;
            let a = 1.0 - t;
            theme::glow(&painter, s.pos, s.size * 2.5, alpha(s.color, 0.35 * a));
            theme::star(&painter, s.pos, s.size * (0.6 + 0.4 * a), alpha(s.color, a));
            t < 1.0
        });
    }
}

fn legend(ui: &mut egui::Ui, color: Color32, label: &str) {
    ui.label(egui::RichText::new(label).size(12.5).color(theme::TEXT_DIM));
    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, color);
}
