//! Monochrome manga look: palette, fonts, gradient helpers, animated background, mascot.

use eframe::egui::{
    self, epaint::Mesh, Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2,
};
use std::f32::consts::{PI, TAU};

// ---------- palette ----------
pub const BG_TOP: Color32 = Color32::from_rgb(6, 6, 8);
pub const BG_BOTTOM: Color32 = Color32::from_rgb(30, 30, 34);
pub const ACCENT: Color32 = Color32::from_rgb(245, 245, 245);
pub const LIGHT: Color32 = Color32::from_rgb(210, 210, 214);
pub const SILVER: Color32 = Color32::from_rgb(150, 150, 156);
pub const GLOW: Color32 = Color32::from_rgb(255, 255, 255);
pub const STEEL: Color32 = Color32::from_rgb(98, 98, 104);
pub const GLINT: Color32 = Color32::from_rgb(230, 230, 230);
pub const TEXT: Color32 = Color32::from_rgb(236, 236, 238);
pub const TEXT_DIM: Color32 = Color32::from_rgb(140, 140, 146);

pub const INK: Color32 = Color32::from_rgb(12, 12, 14);

pub fn luma(c: Color32) -> f32 {
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (255.0 * a) as u8)
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

// ---------- fonts & style ----------

fn read_first(paths: &[String]) -> std::io::Result<Vec<u8>> {
    let mut last = Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    for p in paths {
        last = std::fs::read(p);
        if last.is_ok() {
            break;
        }
    }
    last
}

/// System fonts with Japanese glyphs: Noto Sans CJK on Linux, Yu Gothic / Meiryo on Windows.
fn cjk_font_paths() -> (Vec<String>, Vec<String>) {
    let noto = "/usr/share/fonts/noto-cjk";
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let regular = vec![
        format!("{noto}/NotoSansCJK-Medium.ttc"),
        format!("{noto}/NotoSansCJK-Regular.ttc"),
        format!("{win}\\Fonts\\YuGothM.ttc"),
        format!("{win}\\Fonts\\meiryo.ttc"),
        format!("{win}\\Fonts\\msgothic.ttc"),
    ];
    let bold = vec![
        format!("{noto}/NotoSansCJK-Bold.ttc"),
        format!("{win}\\Fonts\\YuGothB.ttc"),
        format!("{win}\\Fonts\\meiryob.ttc"),
    ];
    (regular, bold)
}

/// Loads a system CJK font (for Japanese labels). Returns true if found.
pub fn setup(ctx: &egui::Context) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let (regular_paths, bold_paths) = cjk_font_paths();
    let regular = read_first(&regular_paths);
    let boldf = read_first(&bold_paths);
    let have_cjk = regular.is_ok();

    let defaults = fonts.families[&FontFamily::Proportional].clone();
    if let Ok(bytes) = regular {
        // Index 0 of each collection is the Japanese face.
        fonts.font_data.insert("noto".into(), egui::FontData::from_owned(bytes).into());
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "noto".into());
    }
    let mut bold_family = defaults;
    if let Ok(bytes) = boldf {
        fonts.font_data.insert("noto-bold".into(), egui::FontData::from_owned(bytes).into());
        bold_family.insert(0, "noto-bold".into());
    } else if have_cjk {
        bold_family.insert(0, "noto".into());
    }
    // Built-in Hack font as the last fallback: it has arrows (← ↑ → ↓ ⇄) that some systems lack.
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("Hack".into());
    bold_family.push("Hack".into());
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = Color32::from_rgb(24, 24, 26);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(105, 105, 112);
    v.selection.stroke = Stroke::new(1.0, Color32::BLACK);
    v.hyperlink_color = GLOW;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    for (w, fill) in [
        (&mut v.widgets.inactive, Color32::from_rgba_unmultiplied(48, 48, 52, 210)),
        (&mut v.widgets.hovered, Color32::from_rgba_unmultiplied(72, 72, 78, 225)),
        (&mut v.widgets.active, Color32::from_rgba_unmultiplied(105, 105, 112, 240)),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = 10.into();
    }
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, alpha(LIGHT, 0.15));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, alpha(ACCENT, 0.7));
    v.widgets.active.bg_stroke = Stroke::new(1.5, ACCENT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, alpha(LIGHT, 0.12));
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 6.0);
    style.spacing.slider_width = 260.0;
    style.spacing.interact_size.y = 26.0;
    // Always dark, whatever the OS theme is (Windows "Light" mode turned the header white),
    // and give both of egui's theme slots our style so nothing can switch it back.
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
    // Dark window title bar too (Windows 10/11).
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
    have_cjk
}

/// Frosted "glass" card.
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(16, 16, 18, 200))
        .stroke(Stroke::new(1.0, alpha(LIGHT, 0.18)))
        .corner_radius(18)
        .inner_margin(18)
        .shadow(egui::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(110) })
}

/// Card header: bold title + optional Japanese subtitle.
pub fn card_title(ui: &mut Ui, title: &str, jp: &str, cjk: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).font(bold(20.0)).color(Color32::WHITE));
        if cjk && !jp.is_empty() {
            ui.label(egui::RichText::new(jp).size(14.0).color(ACCENT));
        }
    });
    ui.add_space(4.0);
}

// ---------- shape helpers ----------

fn rounded_points(rect: Rect, r: f32) -> Vec<Pos2> {
    let r = r.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let corners = [
        (rect.right_bottom() + Vec2::new(-r, -r), 0.0),
        (rect.left_bottom() + Vec2::new(r, -r), PI / 2.0),
        (rect.left_top() + Vec2::new(r, r), PI),
        (rect.right_top() + Vec2::new(-r, r), PI * 1.5),
    ];
    let mut pts = Vec::with_capacity(32);
    for (c, start) in corners {
        for i in 0..=6 {
            let a = start + (i as f32 / 6.0) * PI / 2.0;
            pts.push(c + Vec2::new(a.cos(), a.sin()) * r);
        }
    }
    pts
}

/// Rounded rect filled with a gradient. `color_at` maps (0..1, 0..1) position to colour.
pub fn gradient_rect(painter: &egui::Painter, rect: Rect, r: f32, color_at: impl Fn(f32, f32) -> Color32) {
    let pts = rounded_points(rect, r);
    let mut mesh = Mesh::default();
    let uv = |p: Pos2| ((p.x - rect.left()) / rect.width(), (p.y - rect.top()) / rect.height());
    let (cx, cy) = uv(rect.center());
    mesh.colored_vertex(rect.center(), color_at(cx, cy));
    for p in &pts {
        let (u, v) = uv(*p);
        mesh.colored_vertex(*p, color_at(u, v));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

pub fn vgradient(painter: &egui::Painter, rect: Rect, r: f32, top: Color32, bottom: Color32) {
    gradient_rect(painter, rect, r, |_, v| lerp_color(top, bottom, v));
}

pub fn hgradient(painter: &egui::Painter, rect: Rect, r: f32, left: Color32, right: Color32) {
    gradient_rect(painter, rect, r, |u, _| lerp_color(left, right, u));
}

/// Soft radial glow: solid in the centre fading to transparent.
pub fn glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    let n = 40u32;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        mesh.colored_vertex(center + Vec2::angled(a) * radius, Color32::TRANSPARENT);
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}

/// Glow around a rounded rect (stacked translucent outlines).
pub fn rect_glow(painter: &egui::Painter, rect: Rect, r: f32, color: Color32, strength: f32) {
    for i in 1..=6 {
        let e = i as f32 * 2.0;
        let a = strength * 0.22 * (1.0 - i as f32 / 7.0);
        painter.rect_filled(rect.expand(e), r + e, alpha(color, a));
    }
}

/// 4-point twinkle star.
pub fn star(painter: &egui::Painter, c: Pos2, size: f32, color: Color32) {
    let pts: Vec<Pos2> = (0..8)
        .map(|i| {
            let a = i as f32 * PI / 4.0 - PI / 2.0;
            let r = if i % 2 == 0 { size } else { size * 0.3 };
            c + Vec2::angled(a) * r
        })
        .collect();
    // Not strictly convex, but the fan from the centre renders it correctly.
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, color);
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    for i in 0..8u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 8);
    }
    painter.add(Shape::mesh(mesh));
}

// ---------- widgets ----------

/// Gradient pill button with hover glow and press bounce.
pub fn fancy_button(ui: &mut Ui, label: &str, size: Vec2, c1: Color32, c2: Color32, enabled: bool) -> Response {
    let (rect, resp) = // Sense::CLICK (not click()) so buttons never take keyboard focus: otherwise Space/Enter
    // during the keyboard test would "press" the last-clicked button (e.g. Reset).
    ui.allocate_exact_size(size, if enabled { Sense::CLICK } else { Sense::hover() });
    let ctx = ui.ctx();
    let hover = ctx.animate_bool_with_time(resp.id.with("h"), enabled && resp.hovered(), 0.15);
    let press = ctx.animate_bool_with_time(resp.id.with("p"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let r = rect.shrink(press * 2.0).translate(Vec2::new(0.0, -hover * 2.0 + press * 2.0));
    let p = ui.painter();
    let radius = r.height() / 2.0;
    if enabled {
        rect_glow(p, r, radius, c1, 0.35 + hover * 0.65);
        let lift = |c: Color32| lerp_color(c, Color32::WHITE, hover * 0.18);
        hgradient(p, r, radius, lift(c1), lift(c2));
    } else {
        p.rect_filled(r, radius, Color32::from_rgba_unmultiplied(50, 50, 54, 160));
    }
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        bold(15.0),
        if !enabled { TEXT_DIM } else if luma(c1) + luma(c2) > 330.0 { INK } else { Color32::WHITE },
    );
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

// ---------- animated background ----------

struct Petal {
    pos: Pos2,
    vel: Vec2,
    size: f32,
    rot: f32,
    spin: f32,
    flip: f32,
    flip_speed: f32,
    sway: f32,
}

pub struct Background {
    petals: Vec<Petal>,
    stars: Vec<(f32, f32, f32, f32)>, // x, y (0..1), phase, speed
    rng: u32,
    pub enabled: bool,
    tone: Option<(Rect, std::sync::Arc<Mesh>)>,
}

impl Background {
    pub fn new() -> Self {
        let mut bg = Self { petals: vec![], stars: vec![], rng: 0x9E37_79B9, enabled: true, tone: None };
        for _ in 0..70 {
            let s = (bg.rand(), bg.rand() * 0.75, bg.rand() * TAU, 1.0 + bg.rand() * 2.5);
            bg.stars.push(s);
        }
        bg
    }

    pub fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng % 10_000) as f32 / 10_000.0
    }

    fn spawn(&mut self, screen: Rect, anywhere: bool) -> Petal {
        let x = screen.left() + self.rand() * screen.width() * 1.2;
        let y = if anywhere { screen.top() + self.rand() * screen.height() } else { screen.top() - 20.0 };
        Petal {
            pos: Pos2::new(x, y),
            vel: Vec2::new(-15.0 - self.rand() * 25.0, 25.0 + self.rand() * 35.0),
            size: 5.0 + self.rand() * 7.0,
            rot: self.rand() * TAU,
            spin: (self.rand() - 0.5) * 2.0,
            flip: self.rand() * TAU,
            flip_speed: 1.0 + self.rand() * 2.5,
            sway: self.rand() * TAU,
        }
    }

    pub fn paint(&mut self, ctx: &egui::Context) {
        let screen = ctx.content_rect();
        let (t, dt) = ctx.input(|i| (i.time as f32, i.stable_dt.min(0.05)));
        let p = ctx.layer_painter(egui::LayerId::background());

        // sky gradient
        let mut mesh = Mesh::default();
        mesh.colored_vertex(screen.left_top(), BG_TOP);
        mesh.colored_vertex(screen.right_top(), BG_TOP);
        mesh.colored_vertex(screen.right_bottom(), BG_BOTTOM);
        mesh.colored_vertex(screen.left_bottom(), BG_BOTTOM);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        p.add(Shape::mesh(mesh));

        // drifting aurora blobs
        let w = screen.width();
        let h = screen.height();
        let blob = |x: f32, y: f32| screen.left_top() + Vec2::new(x * w, y * h);
        glow(&p, blob(0.18 + 0.05 * (t * 0.13).sin(), 0.85), w * 0.45, alpha(ACCENT, 0.20));
        glow(&p, blob(0.85, 0.25 + 0.06 * (t * 0.11).cos()), w * 0.40, alpha(STEEL, 0.16));
        glow(&p, blob(0.55 + 0.08 * (t * 0.07).sin(), 0.55), w * 0.30, alpha(SILVER, 0.10));

        // manga screentone: halftone dots fading out from two corners.
        // Built once per window size and reused, so it costs almost nothing per frame.
        if self.tone.as_ref().is_none_or(|(r, _)| *r != screen) {
            self.tone = Some((screen, std::sync::Arc::new(screentone_mesh(screen))));
        }
        if let Some((_, mesh)) = &self.tone {
            p.add(Shape::mesh(mesh.clone()));
        }

        // twinkling stars
        for &(x, y, ph, sp) in &self.stars {
            let a = 0.25 + 0.75 * (0.5 + 0.5 * (t * sp + ph).sin());
            let c = screen.left_top() + Vec2::new(x * w, y * h);
            if sp > 3.0 {
                star(&p, c, 3.5 * a, alpha(Color32::WHITE, a * 0.8));
            } else {
                p.circle_filled(c, 1.2, alpha(Color32::WHITE, a * 0.6));
            }
        }

        if !self.enabled {
            return;
        }
        // sakura petals
        let target = ((w * h) / 22_000.0).clamp(20.0, 70.0) as usize;
        while self.petals.len() < target {
            let anywhere = self.petals.is_empty() || self.petals.len() < target / 2;
            let petal = self.spawn(screen, anywhere);
            self.petals.push(petal);
        }
        for i in 0..self.petals.len() {
            let pt = &mut self.petals[i];
            pt.sway += dt;
            pt.pos += (pt.vel + Vec2::new((pt.sway * 1.3).sin() * 18.0, 0.0)) * dt;
            pt.rot += pt.spin * dt;
            pt.flip += pt.flip_speed * dt;
            if pt.pos.y > screen.bottom() + 20.0 || pt.pos.x < screen.left() - 30.0 {
                self.petals[i] = self.spawn(screen, false);
            }
        }
        for pt in &self.petals {
            draw_petal(&p, pt.pos, pt.size, pt.rot, pt.flip.cos().abs().max(0.15));
        }
    }
}

fn screentone_mesh(screen: Rect) -> Mesh {
    let mut mesh = Mesh::default();
    let color = alpha(Color32::WHITE, 0.07);
    let spacing = 13.0;
    let reach = screen.width().max(screen.height()) * 0.42;
    let corners = [screen.left_bottom(), screen.right_top()];
    let mut y = screen.top();
    let mut row = 0;
    while y < screen.bottom() {
        let mut x = screen.left() + if row % 2 == 0 { 0.0 } else { spacing / 2.0 };
        while x < screen.right() {
            let pt = Pos2::new(x, y);
            let k = corners.iter().map(|c| 1.0 - c.distance(pt) / reach).fold(0.0f32, f32::max);
            if k > 0.05 {
                // small hexagon dot
                let r = 0.4 + k * 2.6;
                let base = mesh.vertices.len() as u32;
                mesh.colored_vertex(pt, color);
                for i in 0..6 {
                    mesh.colored_vertex(pt + Vec2::angled(i as f32 * TAU / 6.0) * r, color);
                }
                for i in 0..6u32 {
                    mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % 6);
                }
            }
            x += spacing;
        }
        y += spacing * 0.87;
        row += 1;
    }
    mesh
}

fn draw_petal(p: &egui::Painter, c: Pos2, size: f32, rot: f32, squash: f32) {
    // Petal outline: rounded teardrop with a small notch at the tip.
    let n = 18u32;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, alpha(Color32::from_rgb(250, 250, 250), 0.80));
    let (s, co) = rot.sin_cos();
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let notch = 1.0 - 0.25 * (-((a - PI / 2.0).powi(2)) * 30.0).exp();
        let r = size * (0.55 + 0.45 * (a / 2.0).sin()) * notch;
        let local = Vec2::new(a.cos() * r * 0.65 * squash, a.sin() * r);
        let rotated = Vec2::new(local.x * co - local.y * s, local.x * s + local.y * co);
        mesh.colored_vertex(c + rotated, alpha(Color32::from_rgb(170, 170, 176), 0.65));
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

// ---------- mascot ----------

/// "Neko-chan", a little original cat mascot who reacts to what you do.
pub struct Mascot {
    msg: String,
    msg_since: f32,
    happy_until: f32,
    now: f32,
}

impl Mascot {
    pub fn new() -> Self {
        Self { msg: "Hi! Let's check your hardware~ ♪".into(), msg_since: 0.0, happy_until: 1.5, now: 0.0 }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        if m != self.msg {
            self.msg = m;
            self.msg_since = self.now;
        }
    }

    pub fn cheer(&mut self) {
        self.happy_until = self.now + 0.6;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.now = ui.input(|i| i.time as f32);
        let t = self.now;
        // Bubble sizes itself to the message and shrinks (or hides) when space is tight.
        let font = FontId::proportional(13.0);
        let full_w = ui.painter().layout_no_wrap(self.msg.clone(), font.clone(), Color32::WHITE).size().x;
        let avail = ui.available_width();
        let bubble_w = (full_w + 28.0).min(avail - 76.0).min(360.0);
        let wrap = bubble_w - 28.0;
        let full_h = ui.painter().layout(self.msg.clone(), font.clone(), Color32::WHITE, wrap).size().y;
        let show_bubble = bubble_w > 120.0;
        let w = if show_bubble { bubble_w + 76.0 } else { 70.0 };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 64.0), Sense::hover());
        let p = ui.painter();

        if show_bubble {
            // speech bubble (types out the message)
            let chars = ((t - self.msg_since) * 45.0) as usize;
            let shown: String = self.msg.chars().take(chars).collect();
            let pop = ((t - self.msg_since) / 0.18).clamp(0.0, 1.0);
            let bh = (full_h + 16.0).clamp(36.0, 60.0);
            let bubble = Rect::from_min_size(Pos2::new(rect.min.x, rect.center().y - bh / 2.0), Vec2::new(bubble_w, bh));
            let bubble = Rect::from_center_size(bubble.center(), bubble.size() * (0.9 + 0.1 * pop));
            p.rect(bubble.expand(2.0), 18.0, Color32::WHITE, Stroke::NONE, StrokeKind::Inside);
            p.rect(bubble, 16.0, Color32::WHITE, Stroke::new(2.0, INK), StrokeKind::Inside);
            let tail = [bubble.right_center() + Vec2::new(-1.0, -6.0), bubble.right_center() + Vec2::new(10.0, 0.0), bubble.right_center() + Vec2::new(-1.0, 6.0)];
            p.add(Shape::convex_polygon(tail.to_vec(), Color32::WHITE, Stroke::NONE));
            p.line_segment([tail[0], tail[1]], Stroke::new(2.0, INK));
            p.line_segment([tail[1], tail[2]], Stroke::new(2.0, INK));
            let galley = p.layout(shown, font, Color32::from_rgb(12, 12, 12), wrap);
            p.with_clip_rect(bubble.shrink(3.0)).galley(Pos2::new(bubble.left() + 14.0, bubble.center().y - full_h / 2.0), galley, Color32::WHITE);
        }

        // cat
        let bob = (t * 2.2).sin() * 2.0;
        let c = Pos2::new(rect.max.x - 32.0, rect.center().y + 4.0 + bob);
        let happy = t < self.happy_until;
        let head = Color32::from_rgb(252, 252, 252);
        glow(p, c, 44.0, alpha(Color32::WHITE, 0.18));
        for side in [-1.0f32, 1.0] {
            let ear = [c + Vec2::new(side * 22.0, -8.0), c + Vec2::new(side * 18.0, -30.0), c + Vec2::new(side * 4.0, -20.0)];
            p.add(Shape::convex_polygon(ear.to_vec(), head, Stroke::new(2.0, INK)));
            let inner = [c + Vec2::new(side * 18.0, -14.0), c + Vec2::new(side * 17.0, -25.0), c + Vec2::new(side * 9.0, -19.0)];
            p.add(Shape::convex_polygon(inner.to_vec(), LIGHT, Stroke::NONE));
        }
        p.circle(c, 24.0, head, Stroke::new(2.2, INK));
        // blush
        for side in [-1.0f32, 1.0] {
            for k in 0..3 {
                let b = c + Vec2::new(side * 14.0 + (k as f32 - 1.0) * 3.0, 6.0);
                p.line_segment([b + Vec2::new(-1.2, 2.2), b + Vec2::new(1.2, -2.2)], Stroke::new(1.1, alpha(INK, 0.55)));
            }
        }
        // eyes
        let blink = (t % 3.7) > 3.55;
        for side in [-1.0f32, 1.0] {
            let e = c + Vec2::new(side * 9.0, -2.0);
            if happy {
                let s = Stroke::new(2.2, Color32::from_rgb(14, 14, 14));
                p.line_segment([e + Vec2::new(-4.0, 2.0), e + Vec2::new(0.0, -2.5)], s);
                p.line_segment([e + Vec2::new(0.0, -2.5), e + Vec2::new(4.0, 2.0)], s);
            } else if blink {
                p.line_segment([e + Vec2::new(-4.0, 0.0), e + Vec2::new(4.0, 0.0)], Stroke::new(2.0, Color32::from_rgb(14, 14, 14)));
            } else {
                p.add(Shape::ellipse_filled(e, Vec2::new(4.0, 5.5), Color32::from_rgb(14, 14, 14)));
                p.add(Shape::ellipse_filled(e + Vec2::new(0.0, 2.0), Vec2::new(3.0, 2.5), alpha(SILVER, 0.8)));
                p.circle_filled(e + Vec2::new(-1.3, -2.2), 1.6, Color32::WHITE);
            }
        }
        // mouth "ω"
        let m = c + Vec2::new(0.0, 7.0);
        let s = Stroke::new(1.4, Color32::from_rgb(14, 14, 14));
        p.line_segment([m + Vec2::new(-4.0, -1.0), m + Vec2::new(-2.0, 1.5)], s);
        p.line_segment([m + Vec2::new(-2.0, 1.5), m + Vec2::new(0.0, -0.5)], s);
        p.line_segment([m + Vec2::new(0.0, -0.5), m + Vec2::new(2.0, 1.5)], s);
        p.line_segment([m + Vec2::new(2.0, 1.5), m + Vec2::new(4.0, -1.0)], s);
    }
}
