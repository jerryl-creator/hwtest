//! Webcam test: live preview, resolution, the real frame rate, and a "too dark" check
//! (covered lens or a closed privacy shutter).
//!
//! Privacy: the camera turns on only when the user presses Start, and turns off again when
//! they press Stop or leave the tab. Frames stay in memory for the preview; nothing is
//! recorded or sent anywhere.

use crate::theme::{self, alpha, Mascot};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

/// Preview frames are scaled down to at most this width (keeps texture uploads cheap).
const PREVIEW_MAX_W: u32 = 960;
/// Average brightness (0–255) below which the picture counts as "dark".
const DARK_LUMA: f32 = 14.0;
/// How long the picture must stay dark before we say so.
const DARK_SECS: f64 = 2.0;

/// A camera the system reports.
#[derive(Clone)]
struct CamInfo {
    name: String,
    id: backend::CamId,
}

/// One preview frame, already converted to RGBA.
struct Frame {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
    /// Average brightness 0–255.
    luma: f32,
}

#[derive(Default)]
struct Shared {
    frame: Option<Frame>,
    /// Bumped for every new frame.
    generation: u64,
    frames: u64,
    fps: f32,
    /// Full capture size and pixel format, e.g. (1280, 720, "MJPEG").
    mode: Option<(u32, u32, String)>,
    error: Option<String>,
    /// The capture thread has finished (error or stopped).
    ended: bool,
}

struct Worker {
    stop: Arc<AtomicBool>,
    shared: Arc<Mutex<Shared>>,
    handle: Option<JoinHandle<()>>,
}

impl Worker {
    fn start(cam: CamInfo) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (s2, sh2) = (stop.clone(), shared.clone());
        let handle = std::thread::Builder::new()
            .name("camera".into())
            .spawn(move || {
                let result = if demo() { demo_capture(&s2, &sh2) } else { capture(cam, &s2, &sh2) };
                let mut sh = sh2.lock().unwrap_or_else(|e| e.into_inner());
                if let Err(e) = result {
                    sh.error = Some(e);
                }
                sh.ended = true;
            })
            .ok();
        Self { stop, shared, handle }
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // Wait for the thread so the device is free if the user starts it again right away.
        // It notices the flag after at most one frame.
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// What the test found, kept after the camera is turned off (for the report).
#[derive(Clone, Default)]
struct Summary {
    name: String,
    size: (u32, u32),
    format: String,
    best_fps: f32,
    frames: u64,
    /// The picture was bright enough at some point.
    saw_light: bool,
}

#[derive(Default)]
pub struct CameraTest {
    cams: Option<Vec<CamInfo>>,
    selected: usize,
    worker: Option<Worker>,
    texture: Option<egui::TextureHandle>,
    seen_generation: u64,
    mirror: bool,
    error: Option<String>,
    /// When the picture last had enough light (None = never while running).
    last_light: Option<f64>,
    started_at: f64,
    summary: Option<Summary>,
    /// The preview's current frame size and brightness.
    frame_info: Option<(u32, u32, f32)>,
    dev_started: bool,
}

fn demo() -> bool {
    std::env::var("NYANSPECT_CAMERA_DEMO").is_ok()
}

impl CameraTest {
    pub fn running(&self) -> bool {
        self.worker.is_some()
    }

    /// Turn the camera off (called when leaving the tab, too).
    pub fn stop(&mut self) {
        if let Some(mut w) = self.worker.take() {
            w.shutdown();
        }
        self.frame_info = None;
    }

    fn refresh(&mut self) {
        self.cams = Some(list_cameras());
        if self.selected >= self.cams.as_ref().map_or(0, |c| c.len()) {
            self.selected = 0;
        }
    }

    fn start(&mut self, now: f64) {
        self.stop();
        let Some(cam) = self.cams.as_ref().and_then(|c| c.get(self.selected)).cloned() else { return };
        self.error = None;
        self.last_light = None;
        self.started_at = now;
        self.summary = Some(Summary { name: cam.name.clone(), ..Default::default() });
        self.worker = Some(Worker::start(cam));
    }

    /// Copy news from the capture thread: a new frame, the frame rate, errors.
    fn pull(&mut self, ctx: &egui::Context, now: f64) {
        let Some(w) = &self.worker else { return };
        let mut sh = w.shared.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = sh.error.take() {
            self.error = Some(e);
        }
        let ended = sh.ended;
        if let Some(s) = self.summary.as_mut() {
            if let Some((fw, fh, f)) = &sh.mode {
                s.size = (*fw, *fh);
                s.format = f.clone();
            }
            s.frames = sh.frames;
            // Only trust the rate once it has had a second to settle.
            if now - self.started_at > 1.5 {
                s.best_fps = s.best_fps.max(sh.fps);
            }
        }
        if sh.generation != self.seen_generation {
            self.seen_generation = sh.generation;
            if let Some(f) = sh.frame.take() {
                let img = egui::ColorImage::from_rgba_unmultiplied([f.w as usize, f.h as usize], &f.rgba);
                match &mut self.texture {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => self.texture = Some(ctx.load_texture("camera-preview", img, egui::TextureOptions::LINEAR)),
                }
                self.frame_info = Some((f.w, f.h, f.luma));
                if f.luma >= DARK_LUMA {
                    self.last_light = Some(now);
                    if let Some(s) = self.summary.as_mut() {
                        s.saw_light = true;
                    }
                }
            }
        }
        drop(sh);
        if ended {
            self.worker = None;
            self.frame_info = None;
        } else {
            ctx.request_repaint();
        }
    }

    /// The picture has been dark for a while.
    fn is_dark(&self, now: f64) -> bool {
        self.running() && self.frame_info.is_some() && now - self.last_light.unwrap_or(self.started_at) > DARK_SECS
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        let mut lines = Vec::new();
        let status = match (&self.summary, &self.error) {
            (_, Some(e)) => {
                lines.push(format!("Couldn't open the camera: {e}"));
                Status::Issue
            }
            (None, None) => {
                if self.cams.as_ref().is_some_and(|c| c.is_empty()) {
                    lines.push("No camera found".into());
                }
                Status::NotTested
            }
            (Some(s), None) => {
                lines.push(format!("Camera: {}", s.name));
                if s.frames == 0 {
                    lines.push("No picture received yet".into());
                    Status::InProgress
                } else {
                    lines.push(format!("Picture: {}×{} {}", s.size.0, s.size.1, s.format));
                    if s.best_fps > 0.0 {
                        lines.push(format!("Frame rate: {:.0} fps (measured)", s.best_fps));
                    }
                    if s.saw_light {
                        Status::Pass
                    } else {
                        lines.push("The picture stayed dark: is the lens covered or the privacy shutter closed?".into());
                        Status::Issue
                    }
                }
            }
        };
        Section { title: "Camera", jp: "カメラ", status, lines }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        let ctx = ui.ctx().clone();
        let now = ui.input(|i| i.time);
        if self.cams.is_none() {
            self.refresh();
        }
        if demo() && !self.dev_started {
            self.dev_started = true;
            self.mirror = std::env::var("NYANSPECT_CAMERA_DEMO").as_deref() == Ok("mirror");
            self.start(now);
        }
        self.pull(&ctx, now);
        let cams = self.cams.clone().unwrap_or_default();
        let dark = self.is_dark(now);
        let live = self.running() && self.frame_info.is_some();

        if self.error.is_some() {
            mascot.say("I can't see through the camera… (；ω；)");
        } else if cams.is_empty() {
            mascot.say("No camera found… is it plugged in? (・_・;)");
        } else if dark {
            mascot.say("It's all dark… is the lens covered? (・_・;)");
        } else if live {
            mascot.say("I can see you! Looking good~ (≧▽≦)");
        } else if self.running() {
            mascot.say("Waking the camera up…");
        } else {
            mascot.say("Press Start and say cheese~ ♪");
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Camera", "カメラ", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, c1, c2) = if self.running() {
                        ("■  Stop camera", theme::LIGHT, theme::STEEL)
                    } else {
                        ("●  Start camera", theme::ACCENT, theme::SILVER)
                    };
                    if theme::fancy_button(ui, label, Vec2::new(160.0, 34.0), c1, c2, !cams.is_empty()).clicked() {
                        if self.running() {
                            self.stop();
                        } else {
                            self.start(now);
                        }
                    }
                    ui.add_space(6.0);
                    let mirror = if self.mirror { "⇋ Mirror: on" } else { "⇋ Mirror: off" };
                    if theme::fancy_button(ui, mirror, Vec2::new(130.0, 34.0), theme::SILVER, theme::STEEL, true)
                        .on_hover_text("Flip the picture like a mirror")
                        .clicked()
                    {
                        self.mirror = !self.mirror;
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, "↻", Vec2::new(40.0, 34.0), theme::SILVER, theme::STEEL, !self.running())
                        .on_hover_text("Look for cameras again")
                        .clicked()
                    {
                        self.refresh();
                    }
                });
            });

            if cams.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Camera").size(13.0).color(theme::TEXT_DIM));
                    let names: Vec<String> = cams.iter().map(|c| short(&c.name, 32)).collect();
                    let opts: Vec<(usize, &str, &str)> = names.iter().enumerate().map(|(i, n)| (i, n.as_str(), "")).collect();
                    let before = self.selected;
                    theme::segmented(ui, "cam-pick", &opts, &mut self.selected);
                    if self.selected != before && self.running() {
                        self.start(now);
                    }
                });
            }
            ui.label(
                egui::RichText::new("The camera is on only while you're on this tab and have pressed Start. Nothing is recorded or sent anywhere.")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);

            // Preview: 16:9 box that fills the width, letterboxing the picture inside it.
            let w = ui.available_width();
            let h = (w * 9.0 / 16.0).min(ui.available_height().max(320.0) - 70.0).clamp(240.0, 620.0);
            let (area, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
            let p = ui.painter_at(area);
            p.rect_filled(area, 16.0, Color32::from_rgba_unmultiplied(6, 6, 8, 230));
            match (&self.texture, self.frame_info) {
                (Some(tex), Some((fw, fh, _))) if live => {
                    let scale = (area.width() / fw as f32).min(area.height() / fh as f32);
                    let img = Rect::from_center_size(area.center(), Vec2::new(fw as f32, fh as f32) * scale);
                    let uv = if self.mirror {
                        Rect::from_min_max(Pos2::new(1.0, 0.0), Pos2::new(0.0, 1.0))
                    } else {
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
                    };
                    egui::Image::from_texture(tex).uv(uv).corner_radius(12).paint_at(ui, img);
                }
                _ => {
                    let msg = if let Some(e) = &self.error {
                        format!("Couldn't open the camera\n{}", short(e, 90))
                    } else if cams.is_empty() {
                        "No camera found".to_string()
                    } else if self.running() {
                        "Starting the camera…".to_string()
                    } else {
                        "Press Start to turn on the camera".to_string()
                    };
                    p.text(area.center(), Align2::CENTER_CENTER, msg, theme::bold(18.0), alpha(theme::TEXT_DIM, 0.9));
                    if self.error.is_some() {
                        p.text(
                            area.center() + Vec2::new(0.0, 52.0),
                            Align2::CENTER_CENTER,
                            "Another app may be using it, or camera access is turned off in your privacy settings.",
                            egui::FontId::proportional(13.0),
                            theme::TEXT_DIM,
                        );
                    }
                }
            }
            p.rect_stroke(area, 16.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.15)), StrokeKind::Inside);
            if live {
                // Pulsing "LIVE" dot, top left.
                let pulse = 0.6 + 0.4 * ((now * 3.0).sin() as f32).abs();
                let at = area.left_top() + Vec2::new(26.0, 24.0);
                p.circle_filled(at, 6.0, alpha(Color32::WHITE, pulse));
                p.text(at + Vec2::new(12.0, 0.0), Align2::LEFT_CENTER, "LIVE", theme::bold(13.0), Color32::WHITE);
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let s = self.summary.clone().unwrap_or_default();
                let fps_now = self.worker.as_ref().map_or(0.0, |w| w.shared.lock().map_or(0.0, |sh| sh.fps));
                chip(ui, &if s.size.0 > 0 { format!("{}×{}", s.size.0, s.size.1) } else { "Resolution".into() }, s.size.0 > 0);
                chip(ui, &if s.format.is_empty() { "Format".into() } else { s.format.clone() }, !s.format.is_empty());
                chip(ui, &if live { format!("{fps_now:.0} fps") } else { "Frame rate".into() }, live && fps_now > 0.0);
                chip(ui, "Picture has light", s.saw_light);
                if let Some(name) = cams.get(self.selected).map(|c| c.name.as_str()) {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(name).size(12.5).color(theme::TEXT_DIM));
                }
            });
            if dark {
                ui.label(
                    egui::RichText::new("⚠ The picture is very dark: open the privacy shutter, uncover the lens, or turn on a light. Webcams also slow down in the dark.")
                        .font(theme::bold(13.0))
                        .color(Color32::WHITE),
                );
            }
        });
    }
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
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

// ---------------------------------------------------------------------------------------------
// Capture (background thread)

/// Counts frames over the last second.
struct FpsMeter(VecDeque<Instant>);

impl FpsMeter {
    fn tick(&mut self) -> f32 {
        let now = Instant::now();
        self.0.push_back(now);
        while self.0.front().is_some_and(|t| now.duration_since(*t).as_secs_f32() > 1.0) {
            self.0.pop_front();
        }
        self.0.len() as f32
    }
}

fn publish(shared: &Mutex<Shared>, frame: Frame, fps: f32) {
    let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
    sh.frame = Some(frame);
    sh.generation += 1;
    sh.frames += 1;
    sh.fps = fps;
}

fn list_cameras() -> Vec<CamInfo> {
    if demo() {
        return vec![CamInfo { name: "Demo camera (test pattern)".into(), id: Default::default() }];
    }
    backend::list()
}

fn capture(cam: CamInfo, stop: &AtomicBool, shared: &Mutex<Shared>) -> Result<(), String> {
    backend::capture(cam, stop, shared)
}

/// Frames per second from a frame interval (seconds as a fraction). Webcams often say
/// 333333/10000000 s rather than 1/30 s, so round rather than expect 1/n.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn fps_of(num: u32, den: u32) -> u32 {
    if num == 0 { 0 } else { (den as f64 / num as f64).round() as u32 }
}

/// Linux: Video4Linux2 directly.
#[cfg(target_os = "linux")]
mod backend {
    use super::*;
    use v4l::frameinterval::FrameIntervalEnum;
    use v4l::framesize::FrameSizeEnum;
    use v4l::io::traits::CaptureStream;
    use v4l::prelude::*;
    use v4l::video::Capture;
    use v4l::FourCC;

    pub type CamId = usize;

    fn pixels(f: FourCC) -> Option<Pixels> {
        match &f.repr {
            b"MJPG" => Some(Pixels::Jpeg),
            b"YUYV" => Some(Pixels::Yuyv),
            b"NV12" => Some(Pixels::Nv12),
            b"RGB3" => Some(Pixels::Rgb),
            b"BGR3" => Some(Pixels::Bgr),
            b"GREY" => Some(Pixels::Gray),
            _ => None,
        }
    }

    /// Video devices that can actually capture a picture (skips the "metadata" twin
    /// that many USB webcams also create).
    pub fn list() -> Vec<CamInfo> {
        let mut nodes = v4l::context::enum_devices();
        nodes.sort_by_key(|n| n.index());
        nodes
            .into_iter()
            .filter_map(|n| {
                let dev = Device::new(n.index()).ok()?;
                let usable = dev.enum_formats().ok()?.iter().any(|f| pixels(f.fourcc).is_some());
                usable.then(|| CamInfo { name: n.name().unwrap_or_else(|| format!("Camera {}", n.index())), id: n.index() })
            })
            .collect()
    }

    fn modes(dev: &Device) -> Vec<(Mode, FourCC)> {
        let mut out = Vec::new();
        for f in dev.enum_formats().unwrap_or_default() {
            let Some(pix) = pixels(f.fourcc) else { continue };
            for fs in dev.enum_framesizes(f.fourcc).unwrap_or_default() {
                let (w, h) = match fs.size {
                    FrameSizeEnum::Discrete(d) => (d.width, d.height),
                    FrameSizeEnum::Stepwise(s) => (s.max_width, s.max_height),
                };
                let fps = dev
                    .enum_frameintervals(f.fourcc, w, h)
                    .unwrap_or_default()
                    .iter()
                    .map(|i| match &i.interval {
                        FrameIntervalEnum::Discrete(fr) => fps_of(fr.numerator, fr.denominator),
                        FrameIntervalEnum::Stepwise(st) => fps_of(st.min.numerator, st.min.denominator),
                    })
                    .max()
                    .unwrap_or(0);
                out.push((Mode { w, h, fps, pix }, f.fourcc));
            }
        }
        out
    }

    pub fn capture(cam: CamInfo, stop: &AtomicBool, shared: &Mutex<Shared>) -> Result<(), String> {
        let dev = Device::new(cam.id).map_err(|e| e.to_string())?;
        let modes = modes(&dev);
        let list: Vec<Mode> = modes.iter().map(|m| m.0).collect();
        if let Some(i) = pick_mode(&list) {
            let (m, cc) = modes[i];
            let _ = dev.set_format(&v4l::Format::new(m.w, m.h, cc));
            if m.fps > 0 {
                let _ = dev.set_params(&v4l::video::capture::Parameters::with_fps(m.fps));
            }
        }
        let fmt = dev.format().map_err(|e| e.to_string())?;
        let pix = pixels(fmt.fourcc).ok_or_else(|| format!("unsupported picture format {}", fmt.fourcc))?;
        shared.lock().unwrap_or_else(|e| e.into_inner()).mode = Some((fmt.width, fmt.height, pix.label().into()));
        let mut stream = MmapStream::with_buffers(&dev, v4l::buffer::Type::VideoCapture, 4).map_err(|e| e.to_string())?;
        stream.set_timeout(std::time::Duration::from_secs(3));
        let mut meter = FpsMeter(VecDeque::new());
        while !stop.load(Ordering::Relaxed) {
            let (buf, meta) = stream.next().map_err(|e| format!("no picture from the camera ({e})"))?;
            let used = (meta.bytesused as usize).min(buf.len());
            let data = if used > 0 { &buf[..used] } else { buf };
            // A single broken frame (common right after start) is not fatal.
            if let Ok(frame) = to_preview(data, pix, fmt.width, fmt.height, fmt.stride as usize) {
                publish(shared, frame, meter.tick());
            }
        }
        Ok(())
    }
}

/// Windows: Media Foundation through nokhwa.
#[cfg(windows)]
mod backend {
    use super::*;
    use nokhwa::utils::{FrameFormat, RequestedFormat, RequestedFormatType};

    pub type CamId = nokhwa::utils::CameraIndex;

    const FORMATS: [FrameFormat; 6] =
        [FrameFormat::MJPEG, FrameFormat::YUYV, FrameFormat::NV12, FrameFormat::RAWRGB, FrameFormat::RAWBGR, FrameFormat::GRAY];

    fn pixels(f: FrameFormat) -> Pixels {
        match f {
            FrameFormat::MJPEG => Pixels::Jpeg,
            FrameFormat::YUYV => Pixels::Yuyv,
            FrameFormat::NV12 => Pixels::Nv12,
            FrameFormat::RAWRGB => Pixels::Rgb,
            FrameFormat::RAWBGR => Pixels::Bgr,
            FrameFormat::GRAY => Pixels::Gray,
        }
    }

    pub fn list() -> Vec<CamInfo> {
        nokhwa::query(nokhwa::utils::ApiBackend::Auto)
            .unwrap_or_default()
            .into_iter()
            .map(|info| CamInfo { name: info.human_name(), id: info.index().clone() })
            .collect()
    }

    pub fn capture(cam: CamInfo, stop: &AtomicBool, shared: &Mutex<Shared>) -> Result<(), String> {
        let open = |kind| nokhwa::Camera::new(cam.id.clone(), RequestedFormat::with_formats(kind, &FORMATS));
        let mut camera = open(RequestedFormatType::AbsoluteHighestFrameRate)
            .or_else(|_| open(RequestedFormatType::None))
            .map_err(|e| e.to_string())?;
        // Pick a smooth, sharp mode: at least 25 fps, as big as possible up to 1080p.
        if let Ok(list) = camera.compatible_camera_formats() {
            let modes: Vec<Mode> =
                list.iter().map(|f| Mode { w: f.width(), h: f.height(), fps: f.frame_rate(), pix: pixels(f.format()) }).collect();
            if let Some(i) = pick_mode(&modes) {
                let _ = camera.set_camera_requset(RequestedFormat::with_formats(RequestedFormatType::Exact(list[i]), &FORMATS));
            }
        }
        camera.open_stream().map_err(|e| e.to_string())?;
        let mut meter = FpsMeter(VecDeque::new());
        let mut result = Ok(());
        while !stop.load(Ordering::Relaxed) {
            let buf = match camera.frame() {
                Ok(b) => b,
                Err(e) => {
                    result = Err(e.to_string());
                    break;
                }
            };
            let (w, h) = (buf.resolution().width(), buf.resolution().height());
            let pix = pixels(buf.source_frame_format());
            shared.lock().unwrap_or_else(|e| e.into_inner()).mode.get_or_insert((w, h, pix.label().into()));
            if let Ok(frame) = to_preview(buf.buffer(), pix, w, h, 0) {
                publish(shared, frame, meter.tick());
            }
        }
        let _ = camera.stop_stream();
        result
    }
}

/// Other systems (macOS): not supported yet.
#[cfg(not(any(target_os = "linux", windows)))]
mod backend {
    use super::*;

    pub type CamId = ();

    pub fn list() -> Vec<CamInfo> {
        Vec::new()
    }

    pub fn capture(_: CamInfo, _: &AtomicBool, _: &Mutex<Shared>) -> Result<(), String> {
        Err("The camera test isn't available on this system yet".into())
    }
}

/// NYANSPECT_CAMERA_DEMO: a moving test pattern instead of a real camera (for screenshots).
fn demo_capture(stop: &AtomicBool, shared: &Mutex<Shared>) -> Result<(), String> {
    let (w, h) = (1280u32, 720u32);
    shared.lock().unwrap_or_else(|e| e.into_inner()).mode = Some((w, h, "MJPEG".into()));
    let mut meter = FpsMeter(VecDeque::new());
    let start = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let t = start.elapsed().as_secs_f32();
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        let bars = [[235, 235, 235], [235, 235, 16], [16, 235, 235], [16, 235, 16], [235, 16, 235], [235, 16, 16], [16, 16, 235]];
        let ball = (w as f32 * (0.5 + 0.35 * (t * 1.3).sin()), h as f32 * 0.72);
        for y in 0..h {
            for x in 0..w {
                let c = if y < h * 2 / 3 {
                    bars[(x * 7 / w) as usize]
                } else {
                    let g = (x * 255 / w) as u8;
                    [g, g, g]
                };
                let d = ((x as f32 - ball.0).powi(2) + (y as f32 - ball.1).powi(2)).sqrt();
                let c = if d < 60.0 { [250, 250, 250] } else { c };
                let i = ((y * w + x) * 3) as usize;
                rgb[i..i + 3].copy_from_slice(&c);
            }
        }
        let frame = to_preview(&rgb, Pixels::Rgb, w, h, 0)?;
        publish(shared, frame, meter.tick());
        std::thread::sleep(std::time::Duration::from_millis(33));
    }
    Ok(())
}

/// One capture mode a camera offers.
#[derive(Clone, Copy, Debug)]
struct Mode {
    w: u32,
    h: u32,
    fps: u32,
    pix: Pixels,
}

/// Best mode for a test: smooth (≥ 25 fps) first, then the biggest picture up to 1080p
/// (bigger costs a lot of decoding for no benefit here), then frame rate, then MJPEG.
fn pick_mode(modes: &[Mode]) -> Option<usize> {
    const FULL_HD: u32 = 1920 * 1080;
    modes
        .iter()
        .enumerate()
        .max_by_key(|(_, m)| {
            let area = m.w * m.h;
            (m.fps >= 25, area <= FULL_HD, if area <= FULL_HD { area } else { u32::MAX - area }, m.fps, m.pix == Pixels::Jpeg)
        })
        .map(|(i, _)| i)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pixels {
    Jpeg,
    Yuyv,
    Nv12,
    Rgb,
    Bgr,
    Gray,
}

impl Pixels {
    fn label(self) -> &'static str {
        match self {
            Pixels::Jpeg => "MJPEG",
            Pixels::Yuyv => "YUYV",
            Pixels::Nv12 => "NV12",
            Pixels::Rgb => "RGB",
            Pixels::Bgr => "BGR",
            Pixels::Gray => "Gray",
        }
    }
}

fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let c = y as i32 - 16;
    let d = u as i32 - 128;
    let e = v as i32 - 128;
    let f = |x: i32| ((x + 128) >> 8).clamp(0, 255) as u8;
    [f(298 * c + 409 * e), f(298 * c - 100 * d - 208 * e), f(298 * c + 516 * d)]
}

/// Convert a camera frame into a (smaller) RGBA preview and measure its brightness.
/// `stride` is the bytes per row (0 = no padding).
fn to_preview(buf: &[u8], pix: Pixels, w: u32, h: u32, stride: usize) -> Result<Frame, String> {
    if pix == Pixels::Jpeg {
        let img = image::load_from_memory_with_format(buf, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?.to_rgb8();
        let (jw, jh) = img.dimensions();
        return to_preview(img.as_raw(), Pixels::Rgb, jw, jh, 0);
    }
    let (wu, hu) = (w as usize, h as usize);
    let bpp = match pix {
        Pixels::Yuyv => 2,
        Pixels::Rgb | Pixels::Bgr => 3,
        _ => 1,
    };
    let row = if stride >= wu * bpp { stride } else { wu * bpp };
    let need = match pix {
        Pixels::Nv12 => row * hu * 3 / 2,
        _ => row * hu,
    };
    if w == 0 || h == 0 || buf.len() < need {
        return Err(format!("short frame: {} of {need} bytes", buf.len()));
    }
    let step = w.div_ceil(PREVIEW_MAX_W).max(1) as usize;
    let (ow, oh) = (wu / step, hu / step);
    let mut rgba = Vec::with_capacity(ow * oh * 4);
    let mut sum = 0u64;
    for oy in 0..oh {
        let y = oy * step;
        for ox in 0..ow {
            let x = ox * step;
            let px = match pix {
                Pixels::Yuyv => {
                    let base = y * row + (x / 2) * 4;
                    yuv_to_rgb(buf[base + (x % 2) * 2], buf[base + 1], buf[base + 3])
                }
                Pixels::Nv12 => {
                    let uv = row * hu + (y / 2) * row + (x / 2) * 2;
                    yuv_to_rgb(buf[y * row + x], buf[uv], buf[uv + 1])
                }
                Pixels::Rgb => {
                    let i = y * row + x * 3;
                    [buf[i], buf[i + 1], buf[i + 2]]
                }
                Pixels::Bgr => {
                    let i = y * row + x * 3;
                    [buf[i + 2], buf[i + 1], buf[i]]
                }
                Pixels::Gray | Pixels::Jpeg => {
                    let g = buf[y * row + x];
                    [g, g, g]
                }
            };
            sum += (px[0] as u64 * 77 + px[1] as u64 * 150 + px[2] as u64 * 29) >> 8;
            rgba.extend_from_slice(&[px[0], px[1], px[2], 255]);
        }
    }
    let luma = sum as f32 / (ow * oh).max(1) as f32;
    Ok(Frame { rgba, w: ow as u32, h: oh as u32, luma })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_smooth_full_hd_over_slow_or_huge_modes() {
        let modes = [
            Mode { w: 3840, h: 2160, fps: 30, pix: Pixels::Jpeg },
            Mode { w: 1920, h: 1080, fps: 5, pix: Pixels::Yuyv },
            Mode { w: 1920, h: 1080, fps: 30, pix: Pixels::Jpeg },
            Mode { w: 1280, h: 720, fps: 60, pix: Pixels::Jpeg },
            Mode { w: 640, h: 480, fps: 30, pix: Pixels::Yuyv },
        ];
        assert_eq!(pick_mode(&modes), Some(2));
        // A VGA webcam: take the best it has.
        let vga = [Mode { w: 640, h: 480, fps: 30, pix: Pixels::Yuyv }, Mode { w: 320, h: 240, fps: 30, pix: Pixels::Yuyv }];
        assert_eq!(pick_mode(&vga), Some(0));
    }

    #[test]
    fn yuyv_white_and_black() {
        // Two pixels: white (Y=235) and black (Y=16), neutral colour.
        let buf = [235, 128, 16, 128];
        let f = to_preview(&buf, Pixels::Yuyv, 2, 1, 0).unwrap();
        assert_eq!(&f.rgba[..3], &[255, 255, 255]);
        assert_eq!(&f.rgba[4..7], &[0, 0, 0]);
    }

    #[test]
    fn big_frames_are_scaled_down_and_brightness_measured() {
        let (w, h) = (1920u32, 1080u32);
        let dark = vec![0u8; (w * h) as usize];
        let f = to_preview(&dark, Pixels::Gray, w, h, 0).unwrap();
        assert!(f.w <= PREVIEW_MAX_W && f.h < h);
        assert!(f.luma < DARK_LUMA);
        let bright = vec![200u8; (w * h) as usize];
        assert!(to_preview(&bright, Pixels::Gray, w, h, 0).unwrap().luma > 150.0);
    }

    #[test]
    fn odd_frame_intervals_still_give_a_frame_rate() {
        assert_eq!(fps_of(1, 30), 30);
        assert_eq!(fps_of(333_333, 10_000_000), 30);
        assert_eq!(fps_of(1001, 30_000), 30);
        assert_eq!(fps_of(0, 1), 0);
    }

    #[test]
    fn padded_rows_use_the_stride() {
        // 2×2 gray with 4 bytes per row: the padding bytes (99) must be skipped.
        let buf = [10, 20, 99, 99, 30, 40, 99, 99];
        let f = to_preview(&buf, Pixels::Gray, 2, 2, 4).unwrap();
        assert_eq!([f.rgba[0], f.rgba[4], f.rgba[8], f.rgba[12]], [10, 20, 30, 40]);
    }

    #[test]
    fn short_frames_are_rejected_not_a_panic() {
        assert!(to_preview(&[1, 2, 3], Pixels::Yuyv, 640, 480, 0).is_err());
    }
}

#[cfg(test)]
mod hardware {
    use super::*;

    /// Real-camera check that prints only numbers (no picture is shown or saved):
    /// `cargo test --release -- --ignored real_camera --nocapture`
    #[test]
    #[ignore]
    fn real_camera() {
        let cams = list_cameras();
        for c in &cams {
            println!("camera: {}", c.name);
        }
        let Some(cam) = cams.into_iter().next() else { return };
        let mut w = Worker::start(cam);
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let sh = w.shared.lock().unwrap();
            println!("after 1 s more: frames {} fps {}", sh.frames, sh.fps);
        }
        w.shutdown();
        let sh = w.shared.lock().unwrap();
        println!("mode {:?} frames {} fps {} error {:?}", sh.mode, sh.frames, sh.fps, sh.error);
        if let Some(f) = &sh.frame {
            println!("preview {}x{} brightness {:.0}", f.w, f.h, f.luma);
        }
    }
}
