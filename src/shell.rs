//! The app's frame: left sidebar, slim top bar, the Home page and the Settings page.

use super::{App, RENDERER_INFO, TABS, Tab};
use crate::i18n::{self, Lang, t};
use crate::report::Status;
use crate::theme::{self, alpha, lerp_color};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Below this window width the sidebar shows icons only.
const COMPACT_BELOW: f32 = 1280.0;
pub const TEXT_SIZES: [(f32, &str); 4] = [(0.9, "Small"), (1.0, "Normal"), (1.15, "Large"), (1.3, "Extra large")];

/// Icons-only sidebar: narrow windows (or NYANSPECT_COMPACT for screenshots).
pub fn is_compact(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < COMPACT_BELOW || std::env::var("NYANSPECT_COMPACT").is_ok()
}

/// What each test is for, shown on the Home cards.
fn blurb(tab: Tab) -> &'static str {
    match tab {
        Tab::Keyboard => "Every key, stuck or double-typing keys, rollover",
        Tab::Mouse => "Buttons, scrolling, double-click problems",
        Tab::Gamepad => "Buttons, triggers, stick drift and rumble",
        Tab::Display => "Dead pixels, backlight bleed, banding",
        Tab::Audio => "Each speaker, and your microphone",
        Tab::Camera => "Live picture, resolution and frame rate",
        Tab::System => "Model, serial number, specs and battery health",
        _ => "",
    }
}

/// The small round status mark used in the sidebar and on Home cards.
fn status_mark(p: &egui::Painter, c: Pos2, status: Status) {
    match status {
        Status::Pass => {
            p.circle_filled(c, 8.0, Color32::WHITE);
            p.text(c, Align2::CENTER_CENTER, "✔", theme::bold(10.0), theme::INK);
        }
        Status::Issue => {
            p.circle_filled(c, 8.0, theme::GLINT);
            p.text(c, Align2::CENTER_CENTER, "!", theme::bold(11.0), theme::INK);
        }
        Status::InProgress => {
            p.circle_stroke(c, 6.5, Stroke::new(2.0, theme::SILVER));
            p.circle_filled(c, 2.5, theme::SILVER);
        }
        Status::NotTested => {}
    }
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Pass => "✔ Looks good",
        Status::Issue => "⚠ Issue found",
        Status::InProgress => "… In progress",
        Status::NotTested => "Not started",
    }
}

impl App {
    /// Work out every test page's result once for this frame.
    pub(crate) fn refresh_statuses(&mut self) {
        self.system.pull();
        let tabs = [Tab::System, Tab::Keyboard, Tab::Mouse, Tab::Gamepad, Tab::Display, Tab::Audio, Tab::Camera];
        self.statuses = tabs.iter().filter_map(|t| self.compute_status(*t).map(|s| (*t, s))).collect();
    }

    /// A page's result this frame (Audio = speakers + microphone). None for pages that aren't tests.
    pub(crate) fn tab_status(&self, tab: Tab) -> Option<Status> {
        self.statuses.iter().find(|(t, _)| *t == tab).map(|(_, s)| *s)
    }

    fn compute_status(&self, tab: Tab) -> Option<Status> {
        Some(match tab {
            Tab::Keyboard => self.keyboard.report().status,
            Tab::Mouse => self.mouse.report().status,
            Tab::Gamepad => self.gamepad.report().status,
            Tab::Display => self.pixels.report().status,
            Tab::Camera => self.camera.report().status,
            Tab::System => self.system.report().status,
            Tab::Audio => {
                let (a, b) = (self.speakers.report().status, self.mic.report().status);
                match (a, b) {
                    _ if a == Status::Issue || b == Status::Issue => Status::Issue,
                    (Status::Pass, Status::Pass) => Status::Pass,
                    (Status::NotTested, Status::NotTested) => Status::NotTested,
                    _ => Status::InProgress,
                }
            }
            Tab::Home | Tab::Report | Tab::Settings => return None,
        })
    }

    /// Start fresh: forget every test result (settings stay).
    pub(crate) fn clear_all_results(&mut self) {
        self.keyboard.reset();
        self.mouse.reset();
        self.gamepad.reset();
        self.pixels.clear_results();
        self.speakers.clear_results();
        self.mic.clear_results();
        self.camera.clear_results();
        self.system.clear_results();
        self.report.quick_done = None;
        self.quick = None;
    }

    pub(crate) fn start_quick_check(&mut self) {
        self.quick = Some(crate::quick::QuickCheck::new());
        self.report.quick_done = None;
        self.tab = Tab::System;
    }

    // ---------------------------------------------------------------- sidebar

    pub(crate) fn sidebar(&mut self, ui: &mut egui::Ui) {
        // Scrolls when the window is too short for every row (small screens, high display scaling).
        let height = ui.available_height();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| self.sidebar_rows(ui, height));
    }

    fn sidebar_rows(&mut self, ui: &mut egui::Ui, height: f32) {
        let compact = is_compact(ui.ctx());
        let w = ui.available_width();
        let top = ui.cursor().top();
        ui.add_space(14.0);

        // Logo
        let (r, logo) = ui.allocate_exact_size(Vec2::new(w, if compact { 40.0 } else { 52.0 }), Sense::hover());
        if let Some(info) = RENDERER_INFO.get() {
            logo.on_hover_text(tr!("Renderer: {0}", info));
        }
        let p = ui.painter();
        let time = ui.input(|i| i.time as f32);
        if compact {
            p.text(r.center(), Align2::CENTER_CENTER, "N", theme::bold(26.0), Color32::WHITE);
            theme::star(p, r.center() + Vec2::new(12.0, -12.0 + (time * 3.0).sin() * 2.0), 5.0, theme::GLINT);
        } else {
            let left = r.left_center() + Vec2::new(14.0, 0.0);
            theme::glow(p, left + Vec2::new(60.0, 0.0), 70.0, alpha(theme::ACCENT, 0.12 + 0.04 * (time * 2.0).sin()));
            let title = p.text(left + Vec2::new(0.0, -8.0), Align2::LEFT_CENTER, "Nyanspect", theme::bold(26.0), Color32::WHITE);
            let sub = if self.cjk { "にゃんスペクト" } else { t("hardware inspector") };
            p.text(left + Vec2::new(2.0, 14.0), Align2::LEFT_CENTER, sub, egui::FontId::proportional(11.0), theme::LIGHT);
            theme::star(p, Pos2::new(title.right() + 6.0, title.top() + 2.0 + (time * 3.0).sin() * 2.0), 6.0, theme::GLINT);
        }
        ui.add_space(12.0);

        // Quick check
        if self.quick.is_none() {
            let label = if compact { "▶" } else { t("▶ Quick check") };
            let b = theme::fancy_button(ui, label, Vec2::new(w - 16.0, 38.0), theme::ACCENT, theme::SILVER, true)
                .on_hover_text(t("A guided tour through every test, one step at a time, ending with your report"));
            if b.clicked() {
                self.start_quick_check();
            }
        } else {
            let (r, _) = ui.allocate_exact_size(Vec2::new(w - 16.0, 38.0), Sense::hover());
            let p = ui.painter();
            p.rect_stroke(r, 19.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.3)), StrokeKind::Inside);
            let label = if compact { "▶" } else { t("Quick check running…") };
            p.text(r.center(), Align2::CENTER_CENTER, label, theme::bold(13.0), theme::TEXT_DIM);
        }
        ui.add_space(14.0);

        for (tab, icon, name, _) in TABS.iter().filter(|t| t.0 != Tab::Settings) {
            if *tab == Tab::Report {
                ui.add_space(8.0);
            }
            self.nav_row(ui, *tab, *icon, name, compact);
        }

        // Settings and language at the bottom.
        let used = ui.cursor().top() - top;
        let bottom = height - used - if compact { 96.0 } else { 52.0 };
        ui.add_space(bottom.max(10.0));
        if compact {
            self.nav_row(ui, Tab::Settings, theme::Icon::Settings, "Settings", true);
            ui.vertical_centered(|ui| self.language_button(ui, true));
        } else {
            ui.horizontal(|ui| {
                let row_w = ui.available_width() - 76.0;
                ui.allocate_ui(Vec2::new(row_w, 42.0), |ui| self.nav_row(ui, Tab::Settings, theme::Icon::Settings, "Settings", false));
                self.language_button(ui, false);
            });
        }
    }

    fn nav_row(&mut self, ui: &mut egui::Ui, tab: Tab, icon: theme::Icon, name: &str, compact: bool) {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 40.0), Sense::CLICK);
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let resp = if compact { resp.on_hover_text(t(name)) } else { resp };
        if resp.clicked() {
            self.tab = tab;
        }
        let sel = self.tab == tab;
        let ctx = ui.ctx();
        let on = ctx.animate_bool_with_time(resp.id.with("sel"), sel, 0.18);
        let hover = ctx.animate_bool_with_time(resp.id.with("hov"), resp.hovered() && !sel, 0.12);
        let p = ui.painter();
        let pill = r.shrink2(Vec2::new(8.0, 2.0));
        if on > 0.01 {
            theme::rect_glow(p, pill, 18.0, theme::ACCENT, 0.3 * on);
            theme::hgradient(p, pill, 18.0, alpha(theme::ACCENT, on), alpha(theme::SILVER, on));
        } else if hover > 0.0 {
            p.rect_filled(pill, 18.0, alpha(Color32::WHITE, 0.07 * hover));
        }
        let col = lerp_color(lerp_color(theme::TEXT_DIM, theme::TEXT, hover), theme::INK, on);
        let status = self.tab_status(tab);
        if compact {
            theme::icon(p, icon, pill.center(), 22.0, col);
            if let Some(s) = status {
                status_mark(p, pill.right_top() + Vec2::new(-8.0, 8.0), s);
            }
        } else {
            theme::icon(p, icon, pill.left_center() + Vec2::new(18.0, 0.0), 19.0, col);
            p.text(pill.left_center() + Vec2::new(38.0, 0.0), Align2::LEFT_CENTER, t(name), theme::bold(14.5), col);
            if let Some(s) = status {
                status_mark(p, pill.right_center() + Vec2::new(-16.0, 0.0), s);
            }
        }
    }

    // ---------------------------------------------------------------- top bar

    pub(crate) fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.set_min_height(64.0);
            ui.add_space(6.0);
            let (_, _, name, jp) = TABS.iter().find(|x| x.0 == self.tab).copied().unwrap_or(TABS[0]);
            ui.label(egui::RichText::new(t(name)).font(theme::bold(24.0)).color(Color32::WHITE));
            if self.cjk {
                ui.label(egui::RichText::new(i18n::subtitle(name, jp)).size(14.0).color(theme::ACCENT));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                if self.show_mascot {
                    self.mascot.ui(ui);
                }
            });
        });
        ui.add_space(4.0);
    }

    // ---------------------------------------------------------------- Home

    pub(crate) fn home_page(&mut self, ui: &mut egui::Ui) {
        let tests = [Tab::System, Tab::Keyboard, Tab::Mouse, Tab::Display, Tab::Audio, Tab::Camera, Tab::Gamepad];
        let statuses: Vec<Status> = tests.iter().map(|t| self.tab_status(*t).unwrap_or(Status::NotTested)).collect();
        let tested = statuses.iter().filter(|s| **s != Status::NotTested).count();
        let issues = statuses.iter().filter(|s| **s == Status::Issue).count();
        self.mascot.say(if tested == 0 {
            t("Welcome! Start the quick check, or pick a test below~ ♪").to_string()
        } else if issues > 0 {
            tr!("{0} thing(s) need a look… see below (；ω；)", issues)
        } else {
            t("Nice progress~ keep going! (≧▽≦)").to_string()
        });

        // Welcome card with the quick check and a system summary.
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let left_w = (ui.available_width() * 0.58).max(360.0);
                ui.allocate_ui(Vec2::new(left_w, 0.0), |ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(t("Welcome to Nyanspect")).font(theme::bold(26.0)).color(Color32::WHITE));
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(t(
                                "Check the battery, keyboard, mouse, screen, speakers, microphone, camera and controller in a few minutes. Not sure where to start? The quick check walks you through everything.",
                            ))
                            .size(14.0)
                            .color(theme::TEXT),
                        );
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            let label = if self.quick.is_some() { t("▶ Continue quick check") } else { t("▶ Start quick check") };
                            if theme::fancy_button(ui, label, Vec2::new(220.0, 46.0), theme::ACCENT, theme::SILVER, true).clicked() {
                                if self.quick.is_none() {
                                    self.start_quick_check();
                                } else if let Some(q) = &self.quick {
                                    self.tab = super::tab_for(q.step());
                                }
                            }
                            ui.add_space(8.0);
                            if outline_button(ui, t("View report")).clicked() {
                                self.tab = Tab::Report;
                            }
                        });
                    });
                });
                ui.add_space(24.0);
                // System summary
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(t("This computer")).font(theme::bold(14.0)).color(theme::TEXT_DIM));
                    ui.add_space(4.0);
                    let screen = ui.ctx().input(|i| i.viewport().monitor_size).map(|s| s * ui.ctx().pixels_per_point());
                    let os = match std::env::consts::OS {
                        "windows" => "Windows",
                        "linux" => "Linux",
                        other => other,
                    };
                    let model = self.system.model();
                    let mut rows = vec![
                        (t("System"), format!("{os} ({})", std::env::consts::ARCH)),
                        (t("Graphics"), RENDERER_INFO.get().cloned().unwrap_or_else(|| "?".into())),
                        (t("Screen"), screen.map_or("?".into(), |s| format!("{:.0} × {:.0}", s.x, s.y))),
                        (t("Version"), env!("CARGO_PKG_VERSION").to_string()),
                    ];
                    if let Some(m) = model {
                        rows.insert(0, (t("Model"), m));
                    }
                    egui::Grid::new("sysinfo").num_columns(2).spacing([14.0, 6.0]).show(ui, |ui| {
                        for (k, v) in rows {
                            ui.label(egui::RichText::new(k).size(13.0).color(theme::TEXT_DIM));
                            ui.label(egui::RichText::new(v).size(13.0).color(Color32::WHITE));
                            ui.end_row();
                        }
                    });
                });
            });

            // Overall progress
            ui.add_space(14.0);
            let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(bar, 4.0, Color32::from_rgb(34, 34, 38));
            let frac = tested as f32 / tests.len() as f32;
            if frac > 0.0 {
                let fill = Rect::from_min_size(bar.min, Vec2::new(bar.width() * frac, bar.height()));
                theme::hgradient(p, fill, 4.0, theme::STEEL, Color32::WHITE);
            }
            ui.add_space(4.0);
            let summary = if issues > 0 {
                tr!("{0} of {1} tests done · {2} with issues", tested, tests.len(), issues)
            } else {
                tr!("{0} of {1} tests done", tested, tests.len())
            };
            ui.label(egui::RichText::new(summary).size(12.5).color(theme::TEXT_DIM));
        });

        ui.add_space(16.0);
        // Test cards: three per row (two on narrow windows).
        let cols = if ui.available_width() > 900.0 { 3 } else { 2 };
        let gap = 14.0;
        let card_w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
        for row in tests.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for tab in row {
                    let i = tests.iter().position(|x| x == tab).unwrap_or(0);
                    if self.test_card(ui, *tab, statuses[i], card_w) {
                        self.tab = *tab;
                    }
                }
            });
            ui.add_space(gap);
        }
    }

    /// One clickable Home card. Returns true when clicked.
    fn test_card(&self, ui: &mut egui::Ui, tab: Tab, status: Status, w: f32) -> bool {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 132.0), Sense::CLICK);
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let ctx = ui.ctx();
        let hover = ctx.animate_bool_with_time(resp.id.with("hov"), resp.hovered(), 0.15);
        let r = r.translate(Vec2::new(0.0, -3.0 * hover));
        let p = ui.painter();
        p.rect_filled(r.translate(Vec2::new(0.0, 6.0)), 18.0, Color32::from_black_alpha(90));
        p.rect_filled(r, 18.0, Color32::from_rgba_unmultiplied(16, 16, 18, 215));
        p.rect_stroke(r, 18.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.18 + 0.35 * hover)), StrokeKind::Inside);
        if hover > 0.01 {
            theme::glow(p, r.left_top() + Vec2::new(44.0, 44.0), 70.0, alpha(theme::ACCENT, 0.10 * hover));
        }
        let (_, icon, name, jp) = TABS.iter().find(|x| x.0 == tab).copied().unwrap_or(TABS[0]);
        // Icon in a circle
        let ic = r.left_top() + Vec2::new(40.0, 42.0);
        p.circle_filled(ic, 22.0, Color32::from_rgb(38, 38, 42));
        p.circle_stroke(ic, 22.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.25)));
        theme::icon(p, icon, ic, 24.0, Color32::WHITE);
        let title = p.text(r.left_top() + Vec2::new(74.0, 32.0), Align2::LEFT_CENTER, t(name), theme::bold(18.0), Color32::WHITE);
        if self.cjk {
            p.text(
                title.right_center() + Vec2::new(8.0, 2.0),
                Align2::LEFT_CENTER,
                i18n::subtitle(name, jp),
                egui::FontId::proportional(12.0),
                theme::ACCENT,
            );
        }
        let galley = p.layout(t(blurb(tab)).to_string(), egui::FontId::proportional(13.0), theme::TEXT_DIM, w - 92.0);
        p.galley(r.left_top() + Vec2::new(74.0, 46.0), galley, theme::TEXT_DIM);
        // Status pill bottom-left, "Open →" bottom-right
        let (text, fill, ink) = match status {
            Status::Pass => (status_text(status), Color32::WHITE, theme::INK),
            Status::Issue => (status_text(status), theme::GLINT, theme::INK),
            Status::InProgress => (status_text(status), alpha(theme::SILVER, 0.35), Color32::WHITE),
            Status::NotTested => (status_text(status), alpha(theme::SILVER, 0.15), theme::TEXT_DIM),
        };
        let galley = p.layout_no_wrap(t(text).to_string(), theme::bold(12.0), ink);
        let pill = Rect::from_min_size(r.left_bottom() + Vec2::new(18.0, -36.0), galley.size() + Vec2::new(18.0, 6.0));
        p.rect_filled(pill, 10.0, fill);
        p.galley(pill.min + Vec2::new(9.0, 3.0), galley, ink);
        p.text(r.right_bottom() + Vec2::new(-20.0, -26.0), Align2::RIGHT_CENTER, t("Open →"), theme::bold(13.0), lerp_color(theme::TEXT_DIM, Color32::WHITE, hover));
        resp.clicked()
    }

    // ---------------------------------------------------------------- Settings

    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        self.mascot.say(t("Make Nyanspect yours~ ♪"));
        let ctx = ui.ctx().clone();
        ui.columns(2, |cols| {
            theme::card().show(&mut cols[0], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Language", "言語", self.cjk);
                let mut lang = i18n::lang();
                let opts: Vec<(Lang, &str, &str)> = Lang::ALL.iter().map(|l| (*l, l.native(), "")).collect();
                if theme::segmented(ui, "set-lang", &opts, &mut lang) {
                    self.set_language(&ctx, lang);
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(t("Until you pick one, Nyanspect follows your system language.")).size(12.5).color(theme::TEXT_DIM));
            });
            cols[0].add_space(14.0);
            theme::card().show(&mut cols[0], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Test results", "結果", self.cjk);
                let now = ui.input(|i| i.time);
                let armed = now < self.clear_armed_until;
                let label = if armed { t("Click again to clear everything") } else { t("Clear all test results") };
                let (c1, c2) = if armed { (theme::ACCENT, theme::SILVER) } else { (theme::SILVER, theme::STEEL) };
                if theme::fancy_button(ui, label, Vec2::new(220.0, 36.0), c1, c2, true).clicked() {
                    if armed {
                        self.clear_all_results();
                        self.clear_armed_until = 0.0;
                        self.cleared_at = Some(now);
                    } else {
                        self.clear_armed_until = now + 4.0;
                    }
                }
                if armed {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                }
                ui.add_space(4.0);
                let note = if self.cleared_at.is_some_and(|at| now - at < 5.0) {
                    t("✔ All results cleared. Ready for the next computer!")
                } else {
                    t("Start fresh, for example before testing another computer. Your settings stay.")
                };
                ui.label(egui::RichText::new(note).size(12.5).color(theme::TEXT_DIM));
            });
            cols[0].add_space(14.0);
            theme::card().show(&mut cols[0], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Text size", "文字サイズ", self.cjk);
                let opts: Vec<(f32, &str, &str)> = TEXT_SIZES.iter().map(|(z, n)| (*z, t(n), "")).collect();
                let mut z = self.ui_scale;
                if theme::segmented(ui, "set-zoom", &opts, &mut z) {
                    self.ui_scale = z;
                    ctx.set_zoom_factor(z);
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(t("Makes everything bigger or smaller, for small laptops or tired eyes.")).size(12.5).color(theme::TEXT_DIM));
            });

            theme::card().show(&mut cols[1], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "Look & sound", "見た目と音", self.cjk);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t("Theme")).font(theme::bold(14.0)).color(Color32::WHITE));
                    ui.add_space(6.0);
                    let opts = [
                        (theme::ThemeChoice::Dark, t("Dark"), t("White ink on black")),
                        (theme::ThemeChoice::Light, t("Light"), t("Black ink on white paper, like a manga page; easier to read in bright rooms")),
                        (theme::ThemeChoice::System, t("Match system"), t("Follow your computer's light / dark setting")),
                    ];
                    theme::segmented(ui, "set-theme", &opts, &mut self.theme);
                });
                ui.add_space(10.0);
                theme::toggle(ui, &mut self.keyboard.sfx.enabled, t("Keyboard sound effects"), t("Anime pop-ups and sounds when you press keys"));
                ui.add_space(8.0);
                if theme::toggle(ui, &mut self.reduce_motion, t("Reduce motion"), t("Stops the falling petals and page animations; also easier on slow PCs")) {
                    self.bg.enabled = !self.reduce_motion;
                }
                ui.add_space(8.0);
                theme::toggle(ui, &mut self.show_mascot, t("Show Neko-chan"), t("The cat who comments on your tests"));
            });
            cols[1].add_space(14.0);
            theme::card().show(&mut cols[1], |ui| {
                ui.set_min_width(ui.available_width());
                theme::card_title(ui, "About", "情報", self.cjk);
                ui.label(egui::RichText::new(tr!("Nyanspect {0} · free and open source (MIT)", env!("CARGO_PKG_VERSION"))).color(theme::TEXT));
                ui.hyperlink_to("github.com/jerryl-creator/nyanspect", "https://github.com/jerryl-creator/nyanspect");
                ui.add_space(4.0);
                ui.label(egui::RichText::new(t("Settings are saved automatically. Nothing you test ever leaves this computer.")).size(12.5).color(theme::TEXT_DIM));
            });
        });
    }
}

/// A flat, outlined secondary button.
fn outline_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let font = theme::bold(15.0);
    let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x + 34.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w.max(140.0), 46.0), Sense::CLICK);
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("h"), resp.hovered(), 0.15);
    let p = ui.painter();
    p.rect_filled(r, 23.0, alpha(Color32::WHITE, 0.04 + 0.06 * hover));
    p.rect_stroke(r, 23.0, Stroke::new(1.2, alpha(theme::LIGHT, 0.35 + 0.4 * hover)), StrokeKind::Inside);
    p.text(r.center(), Align2::CENTER_CENTER, label, font, Color32::WHITE);
    resp
}
