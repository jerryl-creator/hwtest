#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] // no console window on Windows

#[macro_use]
mod i18n;
mod camera;
mod chatter;
mod gamepad;
mod keyboard;
mod layouts;
mod mic;
mod motion;
mod mouse;
mod pixels;
mod quick;
mod report;
mod settings;
mod shell;
mod sfx;
mod sfx_audio;
mod speakers;
mod system;
mod theme;

use eframe::egui::{self, Align2, Color32, Sense, Vec2};
use eframe::{egui_wgpu, wgpu};
use theme::alpha;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Home,
    System,
    Keyboard,
    Mouse,
    Gamepad,
    Display,
    Audio,
    Camera,
    Report,
    Settings,
}

/// Pages: (page, icon, English name, Japanese name). Settings sits at the bottom of the sidebar.
const TABS: &[(Tab, theme::Icon, &str, &str)] = &[
    (Tab::Home, theme::Icon::Home, "Home", "ホーム"),
    (Tab::System, theme::Icon::System, "System", "システム"),
    (Tab::Keyboard, theme::Icon::Keyboard, "Keyboard", "キーボード"),
    (Tab::Mouse, theme::Icon::Mouse, "Mouse", "マウス"),
    (Tab::Gamepad, theme::Icon::Gamepad, "Gamepad", "ゲームパッド"),
    (Tab::Display, theme::Icon::Display, "Display", "ディスプレイ"),
    (Tab::Audio, theme::Icon::Audio, "Audio", "オーディオ"),
    (Tab::Camera, theme::Icon::Camera, "Camera", "カメラ"),
    (Tab::Report, theme::Icon::Report, "Report", "レポート"),
    (Tab::Settings, theme::Icon::Settings, "Settings", "設定"),
];

struct App {
    tab: Tab,
    cjk: bool,
    bg: theme::Background,
    mascot: theme::Mascot,
    keyboard: keyboard::KeyboardTest,
    pixels: pixels::PixelTest,
    speakers: speakers::SpeakerTest,
    mic: mic::MicTest,
    mouse: mouse::MouseTest,
    gamepad: gamepad::GamepadTest,
    camera: camera::CameraTest,
    system: system::SystemPage,
    report: report::ReportTab,
    /// The guided quick check, while it's running.
    quick: Option<quick::QuickCheck>,
    /// Settings page options.
    reduce_motion: bool,
    show_mascot: bool,
    ui_scale: f32,
    pub(crate) theme: theme::ThemeChoice,
    /// Whether the window's title bar was last told to be light.
    light_applied: Option<bool>,
    /// The language the user picked (None = follow the system).
    lang_choice: Option<i18n::Lang>,
    /// "Clear all results" asks for a second click until this time.
    clear_armed_until: f64,
    /// When the results were last cleared (for the confirmation message).
    cleared_at: Option<f64>,
    /// Each page's result, worked out once per frame (the sidebar and Home both show them).
    statuses: Vec<(Tab, report::Status)>,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        // Restore remembered settings (skipped for dev screenshots so they're reproducible).
        let saved = cc
            .storage
            .filter(|_| !dev_run())
            .and_then(|st| eframe::get_value::<settings::Settings>(st, settings::KEY))
            .unwrap_or_default();
        // Language: the user's pick, else the system's. Dev runs: NYANSPECT_LANG=en|zh|ms|ja.
        let lang = match std::env::var("NYANSPECT_LANG") {
            Ok(v) => i18n::Lang::from_locale(&v),
            Err(_) if dev_run() => i18n::Lang::En,
            Err(_) => saved.lang.unwrap_or_else(i18n::Lang::system),
        };
        i18n::set(lang);
        let cjk = theme::setup(&cc.egui_ctx);
        // Dev check: NYANSPECT_LIGHT=1 flips egui to light mode (like Windows' Light theme); the app must still look the same.
        if std::env::var("NYANSPECT_LIGHT").is_ok() {
            cc.egui_ctx.set_theme(egui::Theme::Light);
        }
        let mut app = Self {
            tab: Tab::Home,
            cjk,
            bg: theme::Background::new(),
            mascot: theme::Mascot::new(),
            keyboard: keyboard::KeyboardTest::default(),
            pixels: pixels::PixelTest::default(),
            speakers: speakers::SpeakerTest::new(),
            mic: mic::MicTest::default(),
            mouse: mouse::MouseTest::default(),
            gamepad: gamepad::GamepadTest::default(),
            camera: camera::CameraTest::default(),
            system: system::SystemPage::new(),
            report: report::ReportTab::default(),
            lang_choice: saved.lang,
            quick: None,
            reduce_motion: saved.reduce_motion,
            show_mascot: saved.show_mascot,
            ui_scale: saved.ui_scale,
            theme: dev_theme().unwrap_or(saved.theme),
            light_applied: None,
            clear_armed_until: 0.0,
            cleared_at: None,
            statuses: Vec::new(),
        };
        app.bg.enabled = !app.reduce_motion;
        if (app.ui_scale - 1.0).abs() > 0.01 {
            cc.egui_ctx.set_zoom_factor(app.ui_scale);
        }
        // Ctrl +/- would zoom the app while someone is testing those keys.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        // Dev: NYANSPECT_QUICK=<step 0-7> starts the quick check at that step.
        if let Some(step) = std::env::var("NYANSPECT_QUICK").ok().and_then(|v| v.parse::<usize>().ok()) {
            let mut q = quick::QuickCheck::new();
            q.current = step.min(quick::STEPS.len() - 1);
            app.tab = tab_for(q.step());
            app.quick = Some(q);
        }
        app.keyboard.load_settings(&saved);
        app.pixels.load_settings(&saved);
        app.speakers.load_settings(&saved);
        app.keyboard.apply_env_layout();
        app
    }

    /// A quick-check step's live result: the same status the report shows.
    fn step_status(&self, step: quick::Step) -> report::Status {
        match step {
            quick::Step::System => self.system.report().status,
            quick::Step::Keyboard => self.keyboard.report().status,
            quick::Step::Mouse => self.mouse.report().status,
            quick::Step::Display => self.pixels.report().status,
            quick::Step::Speakers => self.speakers.report().status,
            quick::Step::Microphone => self.mic.report().status,
            quick::Step::Camera => self.camera.report().status,
            quick::Step::Gamepad => self.gamepad.report().status,
        }
    }

    /// "🌐 EN" pill that opens a menu of languages.
    /// The language pill's width (the sidebar leaves room for it next to Settings).
    pub(crate) fn language_button_width(ui: &egui::Ui, compact: bool) -> f32 {
        let label = if compact { "🌐".to_string() } else { format!("🌐 {}", i18n::lang().short()) };
        ui.painter().layout_no_wrap(label, theme::bold(13.0), Color32::WHITE).size().x + 26.0
    }

    pub(crate) fn language_button(&mut self, ui: &mut egui::Ui, compact: bool) {
        let cur = i18n::lang();
        let label = if compact { "🌐".to_string() } else { format!("🌐 {}", cur.short()) };
        let font = theme::bold(13.0);
        let w = Self::language_button_width(ui, compact);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::CLICK);
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(i18n::t("Language"));
        let p = ui.painter();
        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.15);
        p.rect_filled(r, 16.0, Color32::from_rgba_unmultiplied(10, 10, 12, 200));
        p.rect_stroke(r, 16.0, egui::Stroke::new(1.0, alpha(theme::LIGHT, 0.15 + 0.4 * hover)), egui::StrokeKind::Inside);
        p.text(r.center(), Align2::CENTER_CENTER, label, font, theme::lerp_color(theme::TEXT_DIM, Color32::WHITE, hover));
        egui::Popup::menu(&resp).show(|ui| {
            ui.set_min_width(170.0);
            for l in i18n::Lang::ALL {
                let text = if l == cur { format!("✔  {}", l.native()) } else { format!("     {}", l.native()) };
                if ui.add(egui::Button::new(egui::RichText::new(text).size(14.0)).frame(false)).clicked() && l != cur {
                    self.set_language(ui.ctx(), l);
                }
            }
        });
    }

    fn set_language(&mut self, ctx: &egui::Context, lang: i18n::Lang) {
        i18n::set(lang);
        self.lang_choice = Some(lang);
        self.cjk = theme::apply_fonts(ctx, lang);
    }

}

impl eframe::App for App {
    /// Dev helper: with NYANSPECT_DEMO set, inject a few key presses so screenshots show the effects.
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        let t = raw.time.unwrap_or(0.0);
        // NYANSPECT_DISPLAY_DEMO=<mode 0-2>: open the fullscreen display test and click two spots.
        if let Some(mode) = std::env::var("NYANSPECT_DISPLAY_DEMO").ok().and_then(|v| v.parse::<usize>().ok()) {
            use std::sync::atomic::{AtomicU8, Ordering};
            static STEP: AtomicU8 = AtomicU8::new(0);
            let step = STEP.load(Ordering::Relaxed);
            let clicks = [(1.2, egui::pos2(300.0, 200.0)), (1.5, egui::pos2(620.0, 380.0))];
            if step == 0 && t > 0.5 {
                self.pixels.dev_start(mode);
                STEP.store(1, Ordering::Relaxed);
            } else if (1..=2).contains(&step) && t > clicks[step as usize - 1].0 {
                let pos = clicks[step as usize - 1].1;
                raw.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    raw.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
                }
                STEP.store(step + 1, Ordering::Relaxed);
            }
        }
        // NYANSPECT_MOUSE_DEMO: draw a wavy path, click, double-click, scroll, and a chattering right button.
        if std::env::var("NYANSPECT_MOUSE_DEMO").is_ok() && (1.0..2.2).contains(&t) {
            let k = ((t - 1.0) / 1.2) as f32;
            let pos = egui::pos2(420.0 + k * 500.0, 330.0 + (k * 14.0).sin() * 70.0);
            raw.events.push(egui::Event::PointerMoved(pos));
            use std::sync::atomic::{AtomicU8, Ordering};
            static DONE: AtomicU8 = AtomicU8::new(0);
            let step = DONE.load(Ordering::Relaxed);
            let click = |b, n, ev: &mut Vec<egui::Event>| {
                for _ in 0..n {
                    for pressed in [true, false] {
                        ev.push(egui::Event::PointerButton { pos, button: b, pressed, modifiers: Default::default() });
                    }
                }
            };
            // Right twice in one frame (chatter), then two real left clicks 0.3 s apart (a double-click).
            if step == 0 && t > 1.3 {
                click(egui::PointerButton::Secondary, 2, &mut raw.events);
                raw.events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, 1.0), modifiers: Default::default(), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Wheel });
                DONE.store(1, Ordering::Relaxed);
            } else if (step == 1 && t > 1.6) || (step == 2 && t > 1.9) {
                click(egui::PointerButton::Primary, 1, &mut raw.events);
                DONE.store(step + 1, Ordering::Relaxed);
            }
        }
        if std::env::var("NYANSPECT_DEMO").is_err() {
            return;
        }
        // (time, key, presses in the same frame): 2 presses at once simulates a chattering switch.
        let script = [
            (1.2, egui::Key::A, 1),
            (1.5, egui::Key::F5, 1),
            (1.8, egui::Key::Escape, 1),
            (1.9, egui::Key::E, 2),
            (2.1, egui::Key::Space, 1),
            (2.3, egui::Key::F25, 1),
        ];
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        for (i, (at, key, presses)) in script.into_iter().enumerate() {
            if t >= at && NEXT.load(Ordering::Relaxed) == i {
                NEXT.store(i + 1, Ordering::Relaxed);
                for _ in 0..presses {
                    for pressed in [true, false] {
                        raw.events.push(egui::Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: Default::default() });
                    }
                }
            }
        }
    }

    fn persist_egui_memory(&self) -> bool {
        !dev_run()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Dev screenshot runs must never touch the user's saved settings.
        if dev_run() {
            return;
        }
        let mut s = settings::Settings::default();
        self.keyboard.store_settings(&mut s);
        self.pixels.store_settings(&mut s);
        self.speakers.store_settings(&mut s);
        s.lang = self.lang_choice;
        s.reduce_motion = self.reduce_motion;
        s.show_mascot = self.show_mascot;
        s.ui_scale = self.ui_scale;
        s.theme = self.theme;
        eframe::set_value(storage, settings::KEY, &s);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        dev_screenshot(ctx, &mut self.tab);
        // Dev check: NYANSPECT_LANG_SWITCH=zh switches language after 1 s, like using the 🌐 menu.
        if let Ok(v) = std::env::var("NYANSPECT_LANG_SWITCH") {
            let target = i18n::Lang::from_locale(&v);
            if ctx.input(|i| i.time) > 1.0 && i18n::lang() != target {
                self.set_language(ctx, target);
            }
        }
        // Fullscreen display test takes over the whole window.
        if self.pixels.running {
            self.pixels.fullscreen(ctx, ui);
            // Test colours are shown as they are, never flipped by the light theme.
            theme::finish_frame(ctx, false, None);
            return;
        }
        let perf_t0 = std::time::Instant::now();
        self.refresh_statuses();
        self.bg.paint(ctx);
        let perf_bg = perf_t0.elapsed();

        let compact = shell::is_compact(ctx);
        egui::Panel::left("nav")
            .exact_size(if compact { 76.0 } else { 236.0 })
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(8, 8, 10, 190))
                    .stroke(egui::Stroke::new(1.0, alpha(theme::LIGHT, 0.10)))
                    .inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 12 }),
            )
            .show(ui, |ui| self.sidebar(ui));
        egui::Panel::top("top").show_separator_line(false).show(ui, |ui| self.top_bar(ui));
        self.gamepad.poll(ctx);

        if self.tab != Tab::Audio {
            self.speakers.stop();
            // Privacy: the microphone is only ever open while the Audio tab is showing.
            self.mic.stop();
        }
        if self.tab != Tab::Gamepad {
            self.gamepad.stop_rumble();
        }
        if self.tab != Tab::Camera {
            // Privacy: the camera is only ever on while the Camera tab is showing.
            self.camera.stop();
        }

        // Quick check guide bar along the bottom.
        if self.quick.is_some() {
            let step = self.quick.as_ref().map_or(quick::Step::System, |q| q.step());
            let status = self.step_status(step);
            let available = step != quick::Step::Gamepad || self.gamepad.connected();
            let on_tab = self.tab == tab_for(step);
            let mut action = quick::Action::None;
            egui::Panel::bottom("quick")
                .frame(egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 18, top: 4, bottom: 14 }))
                .show(ui, |ui| {
                    if let Some(q) = self.quick.as_mut() {
                        action = q.ui(ui, status, on_tab, available);
                    }
                });
            match action {
                quick::Action::None => {}
                quick::Action::Goto(s) => self.tab = tab_for(s),
                quick::Action::Finish => {
                    let skipped = self.quick.take().map(|q| q.skipped()).unwrap_or_default();
                    self.report.quick_done = Some(skipped.iter().map(|s| s.name()).collect());
                    self.tab = Tab::Report;
                    self.mascot.cheer();
                }
                quick::Action::Exit => self.quick = None,
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 18, top: 6, bottom: 18 }))
            .show(ui, |ui| {
                // Fade + slide the page in when switching tabs.
                let appear = if self.reduce_motion { 1.0 } else { page_appear(ctx, self.tab as u8) };
                ui.set_opacity(appear);
                ui.add_space((1.0 - appear) * 24.0);
                self.keyboard.fit_height = ui.available_height();
                self.mouse.fit_height = ui.available_height();
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| match self.tab {
                    Tab::Home => self.home_page(ui),
                    Tab::Settings => self.settings_page(ui),
                    Tab::Keyboard => self.keyboard.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Mouse => self.mouse.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Gamepad => self.gamepad.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Display => self.pixels.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Camera => self.camera.ui(ui, &mut self.mascot, self.cjk),
                    Tab::System => {
                        let renderer = RENDERER_INFO.get().cloned().unwrap_or_else(|| "?".into());
                        self.system.ui(ui, &mut self.mascot, self.cjk, &renderer);
                    }
                    Tab::Report => {
                        let sections = [
                            self.system.report(),
                            self.keyboard.report(),
                            self.mouse.report(),
                            self.gamepad.report(),
                            self.pixels.report(),
                            self.speakers.report(),
                            self.mic.report(),
                            self.camera.report(),
                        ];
                        let header = report::Header { renderer: RENDERER_INFO.get().cloned().unwrap_or_else(|| "unknown".into()) };
                        self.report.ui(ui, &mut self.mascot, self.cjk, &header, &sections);
                    }
                    Tab::Audio => {
                        self.speakers.ui(ui, &mut self.mascot, self.cjk);
                        ui.add_space(14.0);
                        self.mic.ui(ui, &mut self.mascot, self.cjk, &self.speakers);
                    }
                });
            });
        for sound in self.keyboard.take_sounds() {
            self.speakers.play_sfx(sound);
        }
        // Light theme: turn the finished frame into black ink on white paper.
        let light = self.theme.is_light(ctx);
        theme::finish_frame(ctx, light, self.bg.tone_texture());
        if self.light_applied != Some(light) {
            self.light_applied = Some(light);
            let t = if light { egui::SystemTheme::Light } else { egui::SystemTheme::Dark };
            ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(t));
        }
        perf_report(ctx, perf_bg, perf_t0.elapsed());
        pace_frames(ctx, self.reduce_motion);
    }
}

/// Dev: NYANSPECT_THEME=light|dark|system.
fn dev_theme() -> Option<theme::ThemeChoice> {
    match std::env::var("NYANSPECT_THEME").ok()?.as_str() {
        "light" => Some(theme::ThemeChoice::Light),
        "dark" => Some(theme::ThemeChoice::Dark),
        "system" => Some(theme::ThemeChoice::System),
        _ => None,
    }
}

/// The tab where a quick-check step happens.
fn tab_for(step: quick::Step) -> Tab {
    match step {
        quick::Step::System => Tab::System,
        quick::Step::Keyboard => Tab::Keyboard,
        quick::Step::Mouse => Tab::Mouse,
        quick::Step::Display => Tab::Display,
        quick::Step::Speakers | quick::Step::Microphone => Tab::Audio,
        quick::Step::Camera => Tab::Camera,
        quick::Step::Gamepad => Tab::Gamepad,
    }
}

/// Dev: NYANSPECT_PERF prints, every 3 s, frames drawn and the average time spent in the
/// background and in all of our UI code per frame (egui's tessellation and the GPU come after).
fn perf_report(ctx: &egui::Context, bg: std::time::Duration, total: std::time::Duration) {
    use std::sync::Mutex;
    static ACC: Mutex<(u32, f64, f64, f64)> = Mutex::new((0, 0.0, 0.0, 0.0));
    if std::env::var("NYANSPECT_PERF").is_err() {
        return;
    }
    let now = ctx.input(|i| i.time);
    let mut a = ACC.lock().unwrap();
    a.0 += 1;
    a.1 += bg.as_secs_f64();
    a.2 += total.as_secs_f64();
    if now - a.3 > 3.0 {
        let n = a.0.max(1) as f64;
        eprintln!("perf: {:.1} fps · bg {:.2} ms · ui total {:.2} ms per frame", a.0 as f64 / (now - a.3), a.1 / n * 1e3, a.2 / n * 1e3);
        *a = (0, 0.0, 0.0, now);
    }
}

/// 0→1 easing over ~0.35 s each time the page changes.
fn page_appear(ctx: &egui::Context, page: u8) -> f32 {
    let now = ctx.input(|i| i.time);
    let key = egui::Id::unique("page-switch");
    let (last, since) = ctx.data_mut(|d| *d.get_temp_mut_or(key, (page, now)));
    let since = if last != page {
        ctx.data_mut(|d| d.insert_temp(key, (page, now)));
        now
    } else {
        since
    };
    let t = ((now - since) / 0.35).clamp(0.0, 1.0) as f32;
    1.0 - (1.0 - t).powi(3)
}

/// Adaptive frame rate. Drawing a frame is the app's main cost (≈0.4% of a CPU core per
/// frame-per-second on a typical laptop), and the background only drifts slowly, so:
/// 60 fps right after input (key pop-ups and sparkles need it), 24 fps for the first
/// 10 s of idle, then 12 fps; 8 fps when the window is in the background; 4 fps with
/// "Reduce motion" on; nothing while minimised. Input always repaints immediately, and
/// live pages (camera, mic, playing tones, a connected controller) ask for full speed.
fn pace_frames(ctx: &egui::Context, reduce_motion: bool) {
    let (minimized, focused, now, had_input) = ctx.input(|i| {
        let vp = i.viewport();
        let input = !i.events.is_empty() || i.pointer.is_moving() || i.pointer.any_down();
        (vp.minimized.unwrap_or(false), vp.focused.unwrap_or(true), i.time, input)
    });
    if minimized {
        return;
    }
    let key = egui::Id::unique("last-input");
    if had_input {
        ctx.data_mut(|d| d.insert_temp(key, now));
    }
    let last = ctx.data(|d| d.get_temp::<f64>(key)).unwrap_or(now);
    let quiet = now - last;
    let fps = if quiet < 1.5 {
        60.0
    } else if !focused {
        8.0
    } else if reduce_motion {
        4.0
    } else if quiet < 10.0 {
        24.0
    } else {
        12.0
    };
    ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.0 / fps));
}

/// Which GPU/API ended up drawing the app (shown when hovering the logo).
static RENDERER_INFO: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Pick a GPU that works well everywhere (Intel, AMD, NVIDIA, or software):
/// - prefer the integrated GPU: plenty for a 2D app, and it doesn't wake a laptop's
///   discrete GPU (saves battery and heat);
/// - prefer the most reliable API per OS (DirectX 12 on Windows, Vulkan on Linux);
/// - fall back to a software renderer (WARP / llvmpipe) if there's no usable GPU driver.
fn pick_adapter(adapters: &[wgpu::Adapter], surface: Option<&wgpu::Surface<'_>>) -> Result<wgpu::Adapter, String> {
    use wgpu::{Backend, DeviceType};
    let type_rank = |t: DeviceType| match t {
        DeviceType::IntegratedGpu => 0,
        DeviceType::DiscreteGpu => 1,
        DeviceType::VirtualGpu => 2,
        DeviceType::Other => 3,
        DeviceType::Cpu => 4,
    };
    let api_rank = |b: Backend| match (cfg!(windows), b) {
        (true, Backend::Dx12) => 0,
        (true, Backend::Vulkan) => 1,
        (false, Backend::Vulkan | Backend::Metal) => 0,
        (_, Backend::Gl) => 2,
        _ => 3,
    };
    let best = adapters
        .iter()
        .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
        .min_by_key(|a| {
            let info = a.get_info();
            (type_rank(info.device_type), api_rank(info.backend))
        })
        .cloned()
        .ok_or_else(|| "no GPU adapter can draw to this window".to_string())?;
    let info = best.get_info();
    let name = info.name.replace("(R)", "").replace("(TM)", "").split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = RENDERER_INFO.set(format!("{name} · {:?}", info.backend));
    // Remember every GPU for the System page.
    let seen: Vec<system::AdapterSeen> = adapters
        .iter()
        .map(|a| {
            let i = a.get_info();
            system::AdapterSeen {
                name: i.name,
                vendor: i.vendor,
                device: i.device,
                kind: match i.device_type {
                    DeviceType::IntegratedGpu => Some(system::GpuKind::Integrated),
                    DeviceType::DiscreteGpu => Some(system::GpuKind::Discrete),
                    DeviceType::VirtualGpu | DeviceType::Other => Some(system::GpuKind::Other),
                    DeviceType::Cpu => None,
                },
                gl: i.backend == Backend::Gl,
            }
        })
        .collect();
    let _ = system::GPUS.set(system::unique_gpus(&seen));
    Ok(best)
}

fn run(renderer: eframe::Renderer) -> eframe::Result {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.native_adapter_selector = Some(std::sync::Arc::new(pick_adapter));
    if renderer == eframe::Renderer::Glow {
        let _ = RENDERER_INFO.set("OpenGL".into());
    }
    let options = eframe::NativeOptions {
        renderer,
        multisampling: 0, // egui already anti-aliases its shapes
        persist_window: !dev_run(),
        depth_buffer: 0,
        wgpu_options: egui_wgpu::WgpuConfiguration {
            wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup),
            ..Default::default()
        },
        viewport: egui::ViewportBuilder::default()
            .with_title("Nyanspect")
            .with_app_id("nyanspect")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([960.0, 600.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon-256.png")).expect("icon")),
        ..Default::default()
    };
    eframe::run_native("nyanspect", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

fn main() {
    // Panics are written to the log; the dialog is shown by `fatal` below.
    std::panic::set_hook(Box::new(|info| log_error(&format!("panic: {info}"))));

    let force_gl = std::env::args().any(|a| a == "--opengl");
    let renderer = if force_gl { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu };
    let result = std::panic::catch_unwind(|| run(renderer));
    let err = match result {
        Ok(Ok(())) => return,
        Ok(Err(e)) => e.to_string(),
        Err(_) => "the app crashed while starting".to_string(),
    };

    if force_gl {
        fatal(&format!("Nyanspect could not start its graphics (OpenGL).\n\n{err}"));
        return;
    }
    // The default renderer (DirectX 12 / Vulkan via wgpu) fails on some older PCs and drivers.
    // winit can't create a second event loop in one process, so relaunch ourselves with OpenGL.
    log_error(&format!("wgpu renderer failed: {err}\nRetrying with OpenGL (--opengl)"));
    let relaunch = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).arg("--opengl").status());
    if let Err(e) = relaunch {
        fatal(&format!("Nyanspect could not start its graphics.\n\n{err}\n\nRetry with OpenGL failed: {e}"));
    }
}

fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("nyanspect-error.log")
}

fn log_error(msg: &str) {
    use std::io::Write;
    eprintln!("{msg}");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_path()) {
        let _ = writeln!(f, "{msg}");
    }
}

/// Last resort: log the error and tell the user (a dialog on Windows, where there's no console).
fn fatal(msg: &str) {
    log_error(msg);
    let text = format!("{msg}\n\nDetails were saved to:\n{}", log_path().display());
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(hwnd: *mut core::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        // 0x10 = MB_ICONERROR
        unsafe { MessageBoxW(std::ptr::null_mut(), wide(&text).as_ptr(), wide("Nyanspect").as_ptr(), 0x10) };
    }
    #[cfg(not(windows))]
    eprintln!("{text}");
}

/// True for automated dev runs (screenshots), which must not save or load the user's settings.
fn dev_run() -> bool {
    std::env::var("NYANSPECT_SHOT").is_ok()
}

/// Dev helper: `NYANSPECT_SHOT=out.png NYANSPECT_TAB=<index> nyanspect` saves a window screenshot and exits.
fn dev_screenshot(ctx: &egui::Context, tab: &mut Tab) {
    // NYANSPECT_TAB=<index>; optionally NYANSPECT_TAB2=<index> takes over after 2.4 s (e.g. run a demo, then show the report).
    let time = ctx.input(|i| i.time);
    let pick = if time > 2.4 { std::env::var("NYANSPECT_TAB2").or_else(|_| std::env::var("NYANSPECT_TAB")) } else { std::env::var("NYANSPECT_TAB") };
    if let Ok(t) = pick {
        *tab = TABS[t.parse::<usize>().unwrap_or(0).min(TABS.len() - 1)].0;
    }
    // NYANSPECT_QUIT_AT=<seconds>: close normally (settings get saved) after that long.
    if let Some(q) = std::env::var("NYANSPECT_QUIT_AT").ok().and_then(|v| v.parse::<f64>().ok()) {
        if ctx.input(|i| i.time) > q {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    let Ok(path) = std::env::var("NYANSPECT_SHOT") else { return };
    if let Some((w, h)) = std::env::var("NYANSPECT_SIZE").ok().and_then(|v| v.split_once('x').map(|(w, h)| (w.parse::<f32>(), h.parse::<f32>()))) {
        if let (Ok(w), Ok(h), true) = (w, h, time < 0.5) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
        }
    }
    if std::env::var("NYANSPECT_MAX").is_ok() && time < 0.5 {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    }
    let key = egui::Id::unique("dev-shot-sent");
    let at = std::env::var("NYANSPECT_SHOT_AT").ok().and_then(|v| v.parse().ok()).unwrap_or(2.5);
    if time > at && !ctx.data(|d| d.get_temp::<bool>(key).unwrap_or(false)) {
        ctx.data_mut(|d| d.insert_temp(key, true));
        eprintln!("renderer: {}", RENDERER_INFO.get().map_or("?", |s| s.as_str()));
        let ctx2 = ctx.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
            let [w, h] = img.size;
            let buf = image::RgbaImage::from_raw(w as u32, h as u32, img.as_raw().to_vec()).unwrap();
            buf.save(&path).ok();
            ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx2.request_repaint();
        })));
    }
}
