#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] // no console window on Windows

mod chatter;
mod gamepad;
mod keyboard;
mod layouts;
mod mic;
mod mouse;
mod pixels;
mod report;
mod settings;
mod sfx;
mod sfx_audio;
mod speakers;
mod theme;

use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Vec2};
use eframe::{egui_wgpu, wgpu};
use theme::alpha;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Keyboard,
    Mouse,
    Gamepad,
    Display,
    Audio,
    Report,
}

const TABS: &[(Tab, &str, &str, &str)] = &[
    (Tab::Keyboard, "⌨", "Keyboard", "キーボード"),
    (Tab::Mouse, "🖱", "Mouse", "マウス"),
    (Tab::Gamepad, "🎮", "Gamepad", "ゲームパッド"),
    (Tab::Display, "★", "Display", "ディスプレイ"),
    (Tab::Audio, "🔊", "Audio", "オーディオ"),
    (Tab::Report, "📋", "Report", "レポート"),
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
    report: report::ReportTab,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        let cjk = theme::setup(&cc.egui_ctx);
        // Dev check: NYANSPECT_LIGHT=1 flips egui to light mode (like Windows' Light theme); the app must still look the same.
        if std::env::var("NYANSPECT_LIGHT").is_ok() {
            cc.egui_ctx.set_theme(egui::Theme::Light);
        }
        let mut app = Self {
            tab: Tab::Keyboard,
            cjk,
            bg: theme::Background::new(),
            mascot: theme::Mascot::new(),
            keyboard: keyboard::KeyboardTest::default(),
            pixels: pixels::PixelTest::default(),
            speakers: speakers::SpeakerTest::new(),
            mic: mic::MicTest::default(),
            mouse: mouse::MouseTest::default(),
            gamepad: gamepad::GamepadTest::default(),
            report: report::ReportTab::default(),
        };
        // Restore remembered settings (skipped for dev screenshots so they're reproducible).
        let saved = cc
            .storage
            .filter(|_| !dev_run())
            .and_then(|st| eframe::get_value::<settings::Settings>(st, settings::KEY))
            .unwrap_or_default();
        app.keyboard.load_settings(&saved);
        app.pixels.load_settings(&saved);
        app.speakers.load_settings(&saved);
        app.keyboard.apply_env_layout();
        app
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(6.0);
            // Logo with glow
            let (r, logo) = ui.allocate_exact_size(Vec2::new(170.0, 44.0), Sense::hover());
            if let Some(info) = RENDERER_INFO.get() {
                logo.on_hover_text(format!("Renderer: {info}"));
            }
            let p = ui.painter();
            let t = ui.input(|i| i.time as f32);
            theme::glow(p, r.left_center() + Vec2::new(60.0, 0.0), 70.0, alpha(theme::ACCENT, 0.12 + 0.04 * (t * 2.0).sin()));
            let title = p.text(r.left_center() + Vec2::new(0.0, -6.0), Align2::LEFT_CENTER, "Nyanspect", theme::bold(26.0), Color32::WHITE);
            let sub = if self.cjk { "にゃんスペクト" } else { "hardware inspector" };
            p.text(r.left_center() + Vec2::new(2.0, 15.0), Align2::LEFT_CENTER, sub, egui::FontId::proportional(11.0), theme::LIGHT);
            theme::star(p, Pos2::new(title.right() + 6.0, r.top() + 6.0 + (t * 3.0).sin() * 2.0), 6.0, theme::GLINT);

            ui.add_space(18.0);
            self.tab_bar(ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                self.mascot.ui(ui);
            });
        });
        ui.add_space(8.0);
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        // Tabs share the space: wide with few tabs, narrower as more are added.
        let tab_w = match TABS.len() {
            0..=3 => 138.0,
            4..=5 => 112.0,
            _ => 100.0,
        };
        let font = if TABS.len() <= 3 { 14.5 } else { 13.5 };
        let (bar, _) = ui.allocate_exact_size(Vec2::new(tab_w * TABS.len() as f32 + 12.0, 50.0), Sense::hover());
        let p = ui.painter().clone();
        p.rect_filled(bar, 25.0, Color32::from_rgba_unmultiplied(10, 10, 12, 200));
        p.rect_stroke(bar, 25.0, egui::Stroke::new(1.0, alpha(theme::LIGHT, 0.15)), egui::StrokeKind::Inside);

        let idx = TABS.iter().position(|t| t.0 == self.tab).unwrap_or(0) as f32;
        let x = ui.ctx().animate_value_with_time(egui::Id::unique("tab-slide"), idx, 0.28);
        let pill = Rect::from_min_size(bar.min + Vec2::new(6.0 + x * tab_w, 5.0), Vec2::new(tab_w, 40.0));
        theme::rect_glow(&p, pill, 20.0, theme::ACCENT, 0.45);
        theme::hgradient(&p, pill, 20.0, theme::ACCENT, theme::SILVER);

        for (i, (tab, icon, name, jp)) in TABS.iter().enumerate() {
            let r = Rect::from_min_size(bar.min + Vec2::new(6.0 + i as f32 * tab_w, 5.0), Vec2::new(tab_w, 40.0));
            let resp = ui.interact(r, egui::Id::unique(("tab", i)), Sense::CLICK).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                self.tab = *tab;
            }
            let sel = self.tab == *tab;
            let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() && !sel, 0.15);
            if hover > 0.0 {
                p.rect_filled(r, 20.0, alpha(Color32::WHITE, 0.06 * hover));
            }
            let col = if sel { theme::INK } else { theme::lerp_color(theme::TEXT_DIM, theme::TEXT, hover) };
            let dy = if self.cjk { -6.0 } else { 0.0 };
            p.text(r.center() + Vec2::new(0.0, dy), Align2::CENTER_CENTER, format!("{icon} {name}"), theme::bold(font), col);
            if self.cjk {
                p.text(r.center() + Vec2::new(0.0, 11.0), Align2::CENTER_CENTER, *jp, egui::FontId::proportional(10.0), alpha(col, 0.75));
            }
        }
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
        eframe::set_value(storage, settings::KEY, &s);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        dev_screenshot(ctx, &mut self.tab);
        // Fullscreen display test takes over the whole window.
        if self.pixels.running {
            self.pixels.fullscreen(ctx, ui);
            return;
        }
        self.bg.paint(ctx);

        egui::Panel::top("tabs").show(ui, |ui| self.header(ui));
        self.gamepad.poll(ctx);

        if self.tab != Tab::Audio {
            self.speakers.stop();
            // Privacy: the microphone is only ever open while the Audio tab is showing.
            self.mic.stop();
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::new().inner_margin(egui::Margin { left: 18, right: 18, top: 6, bottom: 18 }))
            .show(ui, |ui| {
                // Fade + slide the page in when switching tabs.
                let appear = page_appear(ctx, self.tab as u8);
                ui.set_opacity(appear);
                ui.add_space((1.0 - appear) * 24.0);
                self.keyboard.fit_height = ui.available_height();
                self.mouse.fit_height = ui.available_height();
                egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui| match self.tab {
                    Tab::Keyboard => self.keyboard.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Mouse => self.mouse.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Gamepad => self.gamepad.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Display => self.pixels.ui(ui, &mut self.mascot, self.cjk),
                    Tab::Report => {
                        let sections = [
                            self.keyboard.report(),
                            self.mouse.report(),
                            self.gamepad.report(),
                            self.pixels.report(),
                            self.speakers.report(),
                            self.mic.report(),
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
        pace_frames(ctx);
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

/// Adaptive frame rate. The background always animates, but slowly, so:
/// ~60 fps right after input (key pop-ups and sparkles need it), ~30 fps when idle,
/// ~15 fps when the window is in the background, and nothing while minimised.
/// Input itself always repaints immediately.
fn pace_frames(ctx: &egui::Context) {
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
    let fps = match (focused, now - last < 1.5) {
        (false, _) => 15.0,
        (true, true) => 60.0,
        (true, false) => 30.0,
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
    let _ = RENDERER_INFO.set(format!("{} · {:?}", info.name, info.backend));
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
