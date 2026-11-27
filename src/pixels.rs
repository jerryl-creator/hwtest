use crate::i18n::t;
use crate::theme::{self, alpha, Mascot};
use eframe::egui::{self, epaint::Mesh, Align2, Color32, Key, Pos2, Rect, Sense, Shape, Stroke, Vec2, ViewportCommand};
use std::time::{Duration, Instant};

/// Which set of fullscreen screens to show.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Solid colours: dead (always black) and stuck (always one colour) pixels.
    DeadPixels,
    /// Black and dark grays: backlight bleed, clouding, uneven brightness.
    Uniformity,
    /// Smooth ramps: banding (visible steps) and colour tints.
    Gradients,
}

#[derive(Clone, Copy)]
enum Fill {
    Solid(Color32),
    /// Left-to-right ramp from black to this colour.
    Ramp(Color32),
}

struct Screen {
    name: &'static str,
    fill: Fill,
}

const fn solid(name: &'static str, c: Color32) -> Screen {
    Screen { name, fill: Fill::Solid(c) }
}

fn screens(mode: Mode) -> Vec<Screen> {
    match mode {
        Mode::DeadPixels => vec![
            solid("Black", Color32::BLACK),
            solid("White", Color32::WHITE),
            solid("Red", Color32::from_rgb(255, 0, 0)),
            solid("Green", Color32::from_rgb(0, 255, 0)),
            solid("Blue", Color32::from_rgb(0, 0, 255)),
            solid("Cyan", Color32::from_rgb(0, 255, 255)),
            solid("Magenta", Color32::from_rgb(255, 0, 255)),
            solid("Yellow", Color32::from_rgb(255, 255, 0)),
            solid("Gray 50%", Color32::from_gray(128)),
        ],
        Mode::Uniformity => vec![
            solid("Black (backlight bleed)", Color32::BLACK),
            solid("Gray 5%", Color32::from_gray(13)),
            solid("Gray 10%", Color32::from_gray(26)),
            solid("Gray 25%", Color32::from_gray(64)),
            solid("Gray 50%", Color32::from_gray(128)),
            solid("Gray 75%", Color32::from_gray(191)),
            solid("White", Color32::WHITE),
        ],
        Mode::Gradients => vec![
            Screen { name: "Gray ramp", fill: Fill::Ramp(Color32::WHITE) },
            Screen { name: "Red ramp", fill: Fill::Ramp(Color32::from_rgb(255, 0, 0)) },
            Screen { name: "Green ramp", fill: Fill::Ramp(Color32::from_rgb(0, 255, 0)) },
            Screen { name: "Blue ramp", fill: Fill::Ramp(Color32::from_rgb(0, 0, 255)) },
        ],
    }
}

impl Mode {
    const ALL: [Mode; 3] = [Mode::DeadPixels, Mode::Uniformity, Mode::Gradients];
    fn label(self) -> &'static str {
        match self {
            Mode::DeadPixels => "Dead pixels",
            Mode::Uniformity => "Backlight & uniformity",
            Mode::Gradients => "Gradients",
        }
    }
    fn hint(self) -> &'static str {
        match self {
            Mode::DeadPixels => "Solid colours: find dead (always black) and stuck (always one colour) pixels",
            Mode::Uniformity => "Black and grays: find backlight bleed, clouding and uneven brightness (best in a dark room)",
            Mode::Gradients => "Smooth ramps: find banding (visible steps) and colour tints",
        }
    }
}

/// A spot the user marked on the screen.
#[derive(Clone)]
pub struct Mark {
    /// Where it was clicked, in window points (for drawing).
    pos: Pos2,
    /// The same spot in real screen pixels (for the report).
    pub px: (u32, u32),
    /// Which screen it was marked on.
    pub on: &'static str,
}

pub struct PixelTest {
    pub running: bool,
    mode: Mode,
    index: usize,
    auto_cycle: bool,
    cycle_secs: f32,
    flash: bool,
    last_switch: Instant,
    show_hint_until: Instant,
    pub marks: Vec<Mark>,
    /// Screen size in real pixels during the last fullscreen test.
    pub screen_px: Option<(u32, u32)>,
    /// Which modes have been run at least once (for the report).
    pub done_modes: Vec<&'static str>,
}

impl Default for PixelTest {
    fn default() -> Self {
        Self {
            running: false,
            mode: Mode::DeadPixels,
            index: 0,
            auto_cycle: false,
            cycle_secs: 3.0,
            flash: false,
            last_switch: Instant::now(),
            show_hint_until: Instant::now(),
            marks: vec![],
            screen_px: None,
            done_modes: vec![],
        }
    }
}

impl PixelTest {
    pub fn load_settings(&mut self, s: &crate::settings::Settings) {
        self.auto_cycle = s.auto_cycle;
        self.cycle_secs = s.cycle_secs.clamp(0.5, 10.0);
    }

    pub fn store_settings(&self, s: &mut crate::settings::Settings) {
        s.auto_cycle = self.auto_cycle;
        s.cycle_secs = self.cycle_secs;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        if self.marks.is_empty() {
            mascot.say(t("Look closely for dots that don't match~ (◎_◎)"));
        } else {
            mascot.say(tr!("{0} spot(s) marked. Check them in the Report tab~", self.marks.len()));
        }
        let time = ui.input(|i| i.time as f32);

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                let opts: Vec<_> = Mode::ALL.iter().map(|m| (*m, t(m.label()), t(m.hint()))).collect();
                if theme::segmented(ui, "disp-mode", &opts, &mut self.mode) {
                    self.index = 0;
                }
            });
            ui.label(egui::RichText::new(t(self.mode.hint())).color(theme::TEXT_DIM));
        });
        ui.add_space(12.0);

        ui.columns(2, |cols| {
            theme::card().show(&mut cols[0], |ui| {
                match self.mode {
                    Mode::DeadPixels => {
                        bullet(ui, theme::LIGHT, t("Dead pixel"), t("always black: easiest to spot on White"));
                        bullet(ui, theme::ACCENT, t("Stuck pixel"), t("always one colour: spot it on Black or the primaries"));
                    }
                    Mode::Uniformity => {
                        bullet(ui, theme::LIGHT, t("Backlight bleed"), t("brighter patches at the edges or corners on Black"));
                        bullet(ui, theme::ACCENT, t("Clouding / uniformity"), t("blotchy or uneven areas on the dark and mid grays"));
                    }
                    Mode::Gradients => {
                        bullet(ui, theme::LIGHT, t("Banding"), t("visible steps or stripes instead of a smooth fade"));
                        bullet(ui, theme::ACCENT, t("Tint"), t("the gray ramp looking pink, green or blue in places"));
                    }
                }
                bullet(ui, theme::GLOW, t("Tip"), t("wipe the screen first so dust doesn't fool you"));
                ui.add_space(12.0);

                ui.label(egui::RichText::new(t("Screens in this test")).font(theme::bold(14.0)));
                ui.add_space(4.0);
                let list = screens(self.mode);
                let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
                let p = ui.painter();
                let step = (row.width() / list.len() as f32).min(56.0);
                for (i, s) in list.iter().enumerate() {
                    let bob = (time * 2.0 + i as f32 * 0.6).sin() * 4.0;
                    let center = Pos2::new(row.left() + step * (i as f32 + 0.5), row.center().y + bob);
                    let r = Rect::from_center_size(center, Vec2::splat(34.0));
                    match s.fill {
                        Fill::Solid(c) => {
                            theme::glow(p, center, 26.0, alpha(if c == Color32::BLACK { theme::SILVER } else { c }, 0.30));
                            p.circle(center, 17.0, c, Stroke::new(2.0, alpha(Color32::WHITE, 0.7)));
                        }
                        Fill::Ramp(c) => {
                            theme::hgradient(p, r, 17.0, Color32::BLACK, c);
                            p.circle_stroke(center, 17.0, Stroke::new(2.0, alpha(Color32::WHITE, 0.7)));
                        }
                    }
                }
            });

            theme::card().show(&mut cols[1], |ui| {
                theme::card_title(ui, "Options", "設定", cjk);
                ui.checkbox(&mut self.auto_cycle, t("Auto-cycle screens"));
                ui.add_enabled(
                    self.auto_cycle,
                    egui::Slider::new(&mut self.cycle_secs, 0.5..=10.0).suffix(" s").text(t("per screen")),
                );
                if self.mode == Mode::DeadPixels {
                    ui.add_space(6.0);
                    ui.checkbox(&mut self.flash, t("Stuck-pixel fixer (rapid RGB flashing)"));
                    if self.flash {
                        ui.label(
                            egui::RichText::new(t("⚠ Flashing lights: skip this if you are sensitive to them."))
                                .size(12.5)
                                .color(theme::GLINT),
                        );
                    }
                }
                ui.add_space(14.0);
                if theme::fancy_button(ui, t("▶  Start fullscreen test"), Vec2::new(260.0, 48.0), theme::ACCENT, theme::SILVER, true).clicked() {
                    self.running = true;
                    self.index = 0;
                    self.last_switch = Instant::now();
                    self.show_hint_until = Instant::now() + Duration::from_secs(4);
                    if !self.done_modes.contains(&self.mode.label()) {
                        self.done_modes.push(self.mode.label());
                    }
                    ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(true));
                }
                ui.add_space(10.0);
                for (k, d) in [
                    (t("Click"), t("mark a bad spot (click it again to remove)")),
                    (t("Space / → / scroll"), t("next screen")),
                    (t("← / right-click"), t("previous screen")),
                    ("Backspace", t("undo last mark")),
                    ("Esc", t("exit")),
                ] {
                    ui.horizontal(|ui| {
                        egui::Frame::new()
                            .fill(alpha(theme::SILVER, 0.2))
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| ui.label(egui::RichText::new(k).size(12.5).color(Color32::WHITE)));
                        ui.label(egui::RichText::new(d).size(12.5).color(theme::TEXT_DIM));
                    });
                }
            });
        });

        if !self.marks.is_empty() {
            ui.add_space(12.0);
            theme::card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::card_title(ui, "Marked spots", "マーク", cjk);
                    ui.label(egui::RichText::new(format!("({})", self.marks.len())).font(theme::bold(18.0)).color(Color32::WHITE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::fancy_button(ui, t("Clear marks"), Vec2::new(120.0, 30.0), theme::SILVER, theme::STEEL, true).clicked() {
                            self.marks.clear();
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    for (i, m) in self.marks.iter().enumerate() {
                        egui::Frame::new()
                            .fill(Color32::WHITE)
                            .corner_radius(10)
                            .inner_margin(egui::Margin::symmetric(8, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(format!("#{} x {}, y {} · {}", i + 1, m.px.0, m.px.1, t(m.on)))
                                        .size(12.5)
                                        .color(theme::INK),
                                );
                            });
                    }
                });
            });
        }
    }

    /// Forget marks and which tests were run.
    pub fn clear_results(&mut self) {
        self.marks.clear();
        self.done_modes.clear();
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        let status = if !self.marks.is_empty() {
            Status::Issue
        } else if self.done_modes.is_empty() {
            Status::NotTested
        } else {
            Status::Pass
        };
        let mut lines = Vec::new();
        if let Some((w, h)) = self.screen_px {
            lines.push(tr!("Screen: {0} × {1} pixels", w, h));
        }
        if !self.done_modes.is_empty() {
            lines.push(tr!("Tests run: {0}", self.done_modes.iter().map(|m| t(m)).collect::<Vec<_>>().join(", ")));
        }
        if self.marks.is_empty() {
            if !self.done_modes.is_empty() {
                lines.push(t("No bad spots marked").into());
            }
        } else {
            for (i, m) in self.marks.iter().enumerate() {
                lines.push(tr!("Marked spot #{0} at x {1}, y {2} (on {3})", i + 1, m.px.0, m.px.1, t(m.on)));
            }
        }
        Section { title: "Display", jp: "ディスプレイ", status, lines }
    }

    /// Dev/screenshots: start the fullscreen test in the given mode.
    pub fn dev_start(&mut self, mode: usize) {
        self.mode = Mode::ALL[mode.min(2)];
        self.running = true;
        self.index = 0;
        self.last_switch = Instant::now();
        self.show_hint_until = Instant::now() + Duration::from_secs(4);
    }

    pub fn fullscreen(&mut self, ctx: &egui::Context, root: &mut egui::Ui) {
        let list = screens(self.mode);
        let n = list.len();
        let (next, prev, exit, undo, click, ppp, scroll) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space) || i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::ArrowLeft) || i.pointer.secondary_clicked(),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::Backspace),
                if i.pointer.primary_clicked() { i.pointer.interact_pos() } else { None },
                i.pixels_per_point,
                i.events
                    .iter()
                    .map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 })
                    .sum::<f32>(),
            )
        });
        let next = next || scroll < 0.0;
        let prev = prev || scroll > 0.0;

        if exit {
            self.running = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            return;
        }
        if next {
            self.index = (self.index + 1) % n;
            self.last_switch = Instant::now();
        }
        if prev {
            self.index = (self.index + n - 1) % n;
            self.last_switch = Instant::now();
        }
        if undo {
            self.marks.pop();
        }
        if self.auto_cycle && self.last_switch.elapsed().as_secs_f32() >= self.cycle_secs {
            self.index = (self.index + 1) % n;
            self.last_switch = Instant::now();
        }
        let flashing = self.flash && self.mode == Mode::DeadPixels;
        let screen = &list[self.index.min(n - 1)];
        let name = t(if flashing { "Flashing" } else { screen.name });

        if let Some(pos) = click {
            // Clicking an existing mark removes it; anywhere else adds one.
            if let Some(i) = self.marks.iter().position(|m| m.pos.distance(pos) < 22.0) {
                self.marks.remove(i);
            } else {
                self.marks.push(Mark { pos, px: ((pos.x * ppp) as u32, (pos.y * ppp) as u32), on: screen.name });
            }
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE).show(root, |ui| {
            let rect = ui.max_rect();
            self.screen_px = Some(((rect.width() * ppp).round() as u32, (rect.height() * ppp).round() as u32));
            ui.allocate_rect(rect, Sense::CLICK);
            let p = ui.painter();
            if flashing {
                let seq = [Color32::RED, Color32::GREEN, Color32::BLUE, Color32::WHITE, Color32::BLACK];
                let t = (ctx.input(|i| i.time) * 30.0) as usize % seq.len();
                p.rect_filled(rect, 0.0, seq[t]);
            } else {
                match screen.fill {
                    Fill::Solid(c) => {
                        p.rect_filled(rect, 0.0, c);
                    }
                    Fill::Ramp(c) => {
                        let mut mesh = Mesh::default();
                        mesh.colored_vertex(rect.left_top(), Color32::BLACK);
                        mesh.colored_vertex(rect.right_top(), c);
                        mesh.colored_vertex(rect.right_bottom(), c);
                        mesh.colored_vertex(rect.left_bottom(), Color32::BLACK);
                        mesh.add_triangle(0, 1, 2);
                        mesh.add_triangle(0, 2, 3);
                        p.add(Shape::mesh(mesh));
                    }
                }
            }

            // Marks: black and white rings so they show on any colour; the centre stays clear.
            for (i, m) in self.marks.iter().enumerate() {
                p.circle_stroke(m.pos, 18.0, Stroke::new(4.0, Color32::BLACK));
                p.circle_stroke(m.pos, 18.0, Stroke::new(2.0, Color32::WHITE));
                for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                    p.line_segment([m.pos + d * 8.0, m.pos + d * 14.0], Stroke::new(2.0, Color32::WHITE));
                }
                let label = m.pos + Vec2::new(22.0, -22.0);
                p.text(label + Vec2::splat(1.0), Align2::LEFT_BOTTOM, format!("{}", i + 1), theme::bold(14.0), Color32::BLACK);
                p.text(label, Align2::LEFT_BOTTOM, format!("{}", i + 1), theme::bold(14.0), Color32::WHITE);
            }

            let left = self.show_hint_until.saturating_duration_since(Instant::now()).as_secs_f32();
            if left > 0.0 {
                let a = (left / 0.3).min(1.0);
                let pill = Rect::from_center_size(rect.center(), Vec2::new(520.0, 74.0));
                p.rect_filled(pill, 37.0, Color32::from_rgba_unmultiplied(12, 12, 12, (225.0 * a) as u8));
                let marks = if self.marks.is_empty() { String::new() } else { tr!("  ·  {0} marked", self.marks.len()) };
                p.text(pill.center() + Vec2::new(0.0, -11.0), Align2::CENTER_CENTER, format!("{} / {}  ·  {name}{marks}", self.index + 1, n), theme::bold(19.0), alpha(Color32::WHITE, a));
                p.text(pill.center() + Vec2::new(0.0, 15.0), Align2::CENTER_CENTER, t("click = mark spot  ·  Space = next  ·  Backspace = undo  ·  Esc = exit"), egui::FontId::proportional(13.0), alpha(theme::LIGHT, a));
            }
        });

        if next || prev || click.is_some() || undo {
            self.show_hint_until = Instant::now() + Duration::from_millis(1200);
        }
        ctx.request_repaint();
    }
}

fn bullet(ui: &mut egui::Ui, color: Color32, title: &str, text: &str) {
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        theme::star(ui.painter(), r.center(), 6.0, color);
        ui.label(egui::RichText::new(title).font(theme::bold(14.0)).color(Color32::WHITE));
        ui.label(egui::RichText::new(text).color(theme::TEXT_DIM));
    });
}
