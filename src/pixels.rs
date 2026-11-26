use crate::theme::{self, alpha, Mascot};
use eframe::egui::{self, Align2, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2, ViewportCommand};
use std::time::{Duration, Instant};

const COLORS: &[(&str, Color32)] = &[
    ("Black", Color32::BLACK),
    ("White", Color32::WHITE),
    ("Red", Color32::from_rgb(255, 0, 0)),
    ("Green", Color32::from_rgb(0, 255, 0)),
    ("Blue", Color32::from_rgb(0, 0, 255)),
    ("Cyan", Color32::from_rgb(0, 255, 255)),
    ("Magenta", Color32::from_rgb(255, 0, 255)),
    ("Yellow", Color32::from_rgb(255, 255, 0)),
    ("Gray 50%", Color32::from_gray(128)),
];

pub struct PixelTest {
    pub running: bool,
    index: usize,
    auto_cycle: bool,
    cycle_secs: f32,
    flash: bool,
    last_switch: Instant,
    show_hint_until: Instant,
}

impl Default for PixelTest {
    fn default() -> Self {
        Self {
            running: false,
            index: 0,
            auto_cycle: false,
            cycle_secs: 3.0,
            flash: false,
            last_switch: Instant::now(),
            show_hint_until: Instant::now(),
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
        mascot.say("Look closely for dots that don't match~ (◎_◎)");
        let t = ui.input(|i| i.time as f32);

        ui.columns(2, |cols| {
            theme::card().show(&mut cols[0], |ui| {
                theme::card_title(ui, "Dead / stuck pixel test", "ドット抜けチェック", cjk);
                ui.label("Fills your whole screen with solid colours, one at a time.");
                ui.add_space(8.0);
                bullet(ui, theme::TEXT_DIM, "Dead pixel", "always black — easiest to spot on White");
                bullet(ui, theme::ACCENT, "Stuck pixel", "always one colour — spot it on Black or the primaries");
                bullet(ui, theme::GLOW, "Tip", "wipe the screen first so dust doesn't fool you");
                ui.add_space(14.0);

                ui.label(egui::RichText::new("Colour sequence").font(theme::bold(14.0)));
                ui.add_space(4.0);
                let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
                let p = ui.painter();
                let step = (row.width() / COLORS.len() as f32).min(56.0);
                for (i, (_, c)) in COLORS.iter().enumerate() {
                    let bob = (t * 2.0 + i as f32 * 0.6).sin() * 4.0;
                    let center = Pos2::new(row.left() + step * (i as f32 + 0.5), row.center().y + bob);
                    theme::glow(p, center, 26.0, alpha(if *c == Color32::BLACK { theme::SILVER } else { *c }, 0.30));
                    p.circle(center, 17.0, *c, Stroke::new(2.0, alpha(Color32::WHITE, 0.7)));
                }
            });

            theme::card().show(&mut cols[1], |ui| {
                theme::card_title(ui, "Options", "設定", cjk);
                ui.checkbox(&mut self.auto_cycle, "Auto-cycle colours");
                ui.add_enabled(
                    self.auto_cycle,
                    egui::Slider::new(&mut self.cycle_secs, 0.5..=10.0).suffix(" s").text("per colour"),
                );
                ui.add_space(8.0);
                ui.checkbox(&mut self.flash, "Stuck-pixel fixer (rapid RGB flashing)");
                if self.flash {
                    ui.label(
                        egui::RichText::new("⚠ Flashing lights — skip this if you are sensitive to them.")
                            .size(12.5)
                            .color(theme::GLINT),
                    );
                }
                ui.add_space(18.0);
                if theme::fancy_button(ui, "▶  Start fullscreen test", Vec2::new(260.0, 48.0), theme::ACCENT, theme::SILVER, true).clicked() {
                    self.running = true;
                    self.index = 0;
                    self.last_switch = Instant::now();
                    self.show_hint_until = Instant::now() + Duration::from_secs(3);
                    ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(true));
                }
                ui.add_space(12.0);
                for (k, d) in [("Click / Space / →", "next colour"), ("Right-click / ←", "previous"), ("Esc", "exit")] {
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
    }

    pub fn fullscreen(&mut self, ctx: &egui::Context, root: &mut egui::Ui) {
        let (next, prev, exit) = ctx.input(|i| {
            (
                i.key_pressed(Key::Space) || i.key_pressed(Key::ArrowRight) || i.pointer.primary_clicked(),
                i.key_pressed(Key::ArrowLeft) || i.pointer.secondary_clicked(),
                i.key_pressed(Key::Escape),
            )
        });

        if exit {
            self.running = false;
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
            return;
        }
        if next {
            self.index = (self.index + 1) % COLORS.len();
            self.last_switch = Instant::now();
        }
        if prev {
            self.index = (self.index + COLORS.len() - 1) % COLORS.len();
            self.last_switch = Instant::now();
        }
        if self.auto_cycle && self.last_switch.elapsed().as_secs_f32() >= self.cycle_secs {
            self.index = (self.index + 1) % COLORS.len();
            self.last_switch = Instant::now();
        }

        let (name, color) = if self.flash {
            let seq = [Color32::RED, Color32::GREEN, Color32::BLUE, Color32::WHITE, Color32::BLACK];
            let t = (ctx.input(|i| i.time) * 30.0) as usize % seq.len();
            ("Flashing", seq[t])
        } else {
            COLORS[self.index]
        };

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(color))
            .show(root, |ui| {
                let rect = ui.max_rect();
                ui.allocate_rect(rect, Sense::CLICK);
                let left = self.show_hint_until.saturating_duration_since(Instant::now()).as_secs_f32();
                if left > 0.0 {
                    let a = (left / 0.3).min(1.0);
                    let pill = Rect::from_center_size(rect.center(), Vec2::new(380.0, 70.0));
                    let p = ui.painter();
                    p.rect_filled(pill, 35.0, Color32::from_rgba_unmultiplied(12, 12, 12, (225.0 * a) as u8));
                    p.text(pill.center() + Vec2::new(0.0, -10.0), Align2::CENTER_CENTER, format!("{} / {}  ·  {name}", self.index + 1, COLORS.len()), theme::bold(20.0), alpha(Color32::WHITE, a));
                    p.text(pill.center() + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, "click / Space = next   ·   Esc = exit", egui::FontId::proportional(13.0), alpha(theme::LIGHT, a));
                }
            });

        if next || prev {
            self.show_hint_until = Instant::now() + Duration::from_millis(900);
        }
        ctx.request_repaint();
    }
}

fn bullet(ui: &mut egui::Ui, color: Color32, title: &str, text: &str) {
    ui.horizontal_wrapped(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
        theme::star(ui.painter(), r.center(), 6.0, if color == theme::TEXT_DIM { theme::LIGHT } else { color });
        ui.label(egui::RichText::new(title).font(theme::bold(14.0)).color(Color32::WHITE));
        ui.label(egui::RichText::new(text).color(theme::TEXT_DIM));
    });
}
