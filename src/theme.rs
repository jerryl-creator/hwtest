//! Monochrome manga look: palette, fonts, gradient helpers, animated background, mascot.

use std::sync::OnceLock;
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

/// Font files are big (≈20 MB), so each is read once and kept for the life of the app
/// (switching language only reorders them).
fn font_bytes(path: &str) -> Option<&'static [u8]> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: OnceLock<Mutex<HashMap<String, Option<&'static [u8]>>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    *cache.entry(path.to_string()).or_insert_with(|| map_font(path))
}

/// Memory-map a font file instead of reading it: the OS then loads only the parts actually
/// used (a few MB of a ~20 MB CJK font) and can share them with other apps. Falls back to
/// reading the file if mapping fails.
fn map_font(path: &str) -> Option<&'static [u8]> {
    let file = std::fs::File::open(path).ok()?;
    // SAFETY: system font files aren't modified while apps use them; the map lives for the
    // rest of the program (leaked on purpose, like the fonts themselves).
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(map) => Some(&**Box::leak(Box::new(map))),
        Err(_) => std::fs::read(path).ok().map(|b| &*Box::leak(b.into_boxed_slice())),
    }
}

/// The first font that exists: (bytes, face index inside a .ttc collection).
fn first_font(candidates: &[(String, u32)]) -> Option<(&'static [u8], u32)> {
    candidates.iter().find_map(|(path, index)| font_bytes(path).map(|b| (b, *index)))
}

/// System CJK fonts: (Japanese regular, Japanese bold, Chinese regular, Chinese bold) candidates.
/// Noto Sans CJK collections hold every region: face 0 is Japanese, face 2 Simplified Chinese.
/// On Windows, Japanese fonts lack many simplified characters, so Chinese uses Microsoft YaHei.
fn cjk_font_paths() -> [Vec<(String, u32)>; 4] {
    let win = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    let noto = ["/usr/share/fonts/noto-cjk", "/usr/share/fonts/opentype/noto", "/usr/share/fonts/google-noto-cjk"];
    let noto_files = |names: &[&str], index: u32| -> Vec<(String, u32)> {
        noto.iter().flat_map(|dir| names.iter().map(move |n| (format!("{dir}/{n}"), index))).collect()
    };
    let regular = ["NotoSansCJK-Medium.ttc", "NotoSansCJK-Regular.ttc"];
    let bold = ["NotoSansCJK-Bold.ttc"];
    let winf = |names: &[&str]| -> Vec<(String, u32)> { names.iter().map(|n| (format!("{win}\\Fonts\\{n}"), 0)).collect() };
    let mut ja = noto_files(&regular, 0);
    ja.extend(winf(&["YuGothM.ttc", "meiryo.ttc", "msgothic.ttc"]));
    let mut ja_bold = noto_files(&bold, 0);
    ja_bold.extend(winf(&["YuGothB.ttc", "meiryob.ttc"]));
    let mut zh = noto_files(&regular, 2);
    zh.extend(winf(&["msyh.ttc", "simsun.ttc"]));
    let mut zh_bold = noto_files(&bold, 2);
    zh_bold.extend(winf(&["msyhbd.ttc"]));
    [ja, ja_bold, zh, zh_bold]
}

/// Set up fonts for a language: the matching CJK font first (so Chinese text gets Chinese
/// glyph shapes and Japanese gets Japanese ones), the other one as a fallback.
/// Returns true if any CJK font was found.
pub fn apply_fonts(ctx: &egui::Context, lang: crate::i18n::Lang) -> bool {
    let [ja, ja_bold, zh, zh_bold] = cjk_font_paths();
    let (ja, ja_bold, zh, zh_bold) = (first_font(&ja), first_font(&ja_bold), first_font(&zh), first_font(&zh_bold));
    let chinese_first = lang == crate::i18n::Lang::Zh;
    let order = |a, b| if chinese_first { [b, a] } else { [a, b] };

    let mut fonts = egui::FontDefinitions::default();
    let defaults = fonts.families[&FontFamily::Proportional].clone();
    let mut regular_names = Vec::new();
    let mut bold_names = Vec::new();
    for (name, font) in order(("cjk-ja", ja), ("cjk-zh", zh)) {
        if let Some((bytes, index)) = font {
            let mut data = egui::FontData::from_static(bytes);
            data.index = index;
            fonts.font_data.insert(name.into(), data.into());
            regular_names.push(name.to_string());
        }
    }
    for (name, font) in order(("cjk-ja-bold", ja_bold), ("cjk-zh-bold", zh_bold)) {
        if let Some((bytes, index)) = font {
            let mut data = egui::FontData::from_static(bytes);
            data.index = index;
            fonts.font_data.insert(name.into(), data.into());
            bold_names.push(name.to_string());
        }
    }
    if bold_names.is_empty() {
        bold_names = regular_names.clone();
    }
    let have_cjk = !regular_names.is_empty();
    let prop = fonts.families.get_mut(&FontFamily::Proportional).unwrap();
    for (i, n) in regular_names.iter().enumerate() {
        prop.insert(i, n.clone());
    }
    // Built-in Hack font as the last fallback: it has arrows (← ↑ → ↓ ⇄) that some systems lack.
    prop.push("Hack".into());
    // The bold family gets its own copies of the shared fallback fonts, because a font's
    // baseline correction depends on which font leads the family.
    let mut bold_family = bold_names;
    for name in defaults.iter().chain(std::iter::once(&"Hack".to_string())) {
        if let Some(data) = fonts.font_data.get(name).cloned() {
            let copy = format!("bold:{name}");
            fonts.font_data.insert(copy.clone(), data);
            bold_family.push(copy);
        }
    }
    fonts.families.insert(FontFamily::Name("bold".into()), bold_family.clone());
    let regular_family = fonts.families[&FontFamily::Proportional].clone();
    align_baselines(&mut fonts, &regular_family, have_cjk);
    align_baselines(&mut fonts, &bold_family, have_cjk);
    ctx.set_fonts(fonts);
    have_cjk
}

/// A font's vertical metrics in ems: ascent, descent (negative), line gap, cap height.
fn vmetrics(data: &egui::FontData) -> Option<[f32; 4]> {
    use skrifa::MetadataProvider;
    let font = skrifa::FontRef::from_index((*data.font).as_ref(), data.index).ok()?;
    let m = font.metrics(skrifa::instance::Size::unscaled(), skrifa::instance::LocationRef::default());
    let em = f32::from(m.units_per_em);
    if em <= 0.0 {
        return None;
    }
    let cap = m.cap_height.filter(|c| *c > 0.0).unwrap_or(0.7 * em);
    Some([m.ascent / em, m.descent / em, m.leading / em, cap / em])
}

/// How far below the row's middle Noto Sans CJK puts the middle of a capital letter, in ems
/// (ascent 1.16, descent -0.288, cap height 0.733). The app's look was tuned with it.
const NOTO_CAP_CENTRE: f32 = 1.16 - 0.733 / 2.0 - (1.16 + 0.288) / 2.0;

/// Shift each font in a family so all of them share one baseline (egui centres fallback
/// glyphs by height, which leaves mixed fonts at different heights: "简" from Microsoft
/// YaHei sat lower than Yu Gothic text on Windows). With `centre`, also shift the leading
/// font so capitals sit in boxes the way Noto Sans CJK's do.
/// Returns each font's offset in ems (positive = down).
fn align_baselines(fonts: &mut egui::FontDefinitions, family: &[String], centre: bool) -> Vec<f32> {
    let metrics: Vec<Option<[f32; 4]>> = family.iter().map(|n| fonts.font_data.get(n).and_then(|d| vmetrics(d))).collect();
    let Some(Some(lead)) = metrics.first().copied() else { return Vec::new() };
    let offsets = baseline_offsets(lead, &metrics, centre);
    for (name, off) in family.iter().zip(&offsets) {
        if let Some(data) = fonts.font_data.get_mut(name) {
            std::sync::Arc::make_mut(data).tweak.y_offset_factor = *off;
        }
    }
    offsets
}

fn baseline_offsets(lead: [f32; 4], metrics: &[Option<[f32; 4]>], centre: bool) -> Vec<f32> {
    let height = |m: [f32; 4]| m[0] - m[1] + m[2];
    let lead_off = if centre { NOTO_CAP_CENTRE - (lead[0] - lead[3] / 2.0 - height(lead) / 2.0) } else { 0.0 };
    metrics
        .iter()
        .map(|m| match m {
            // egui draws a fallback glyph at its own ascent, centred on the lead font's height.
            Some(f) => lead[0] - f[0] - 0.5 * (height(lead) - height(*f)) + lead_off,
            None => lead_off,
        })
        .collect()
}

/// Fonts and the monochrome style. Returns true if a CJK font was found.
pub fn setup(ctx: &egui::Context) -> bool {
    let have_cjk = apply_fonts(ctx, crate::i18n::lang());

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

/// Card header: bold title in the current language + a small subtitle
/// (Japanese under English/Malay, English under Chinese/Japanese). `title` is the English text.
pub fn card_title(ui: &mut Ui, title: &str, jp: &str, cjk: bool) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::t(title)).font(bold(20.0)).color(Color32::WHITE));
        let sub = crate::i18n::subtitle(title, jp);
        if cjk && !jp.is_empty() {
            ui.label(egui::RichText::new(sub).size(14.0).color(ACCENT));
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
    // Grow to fit longer (translated) labels.
    let text_w = ui.painter().layout_no_wrap(label.to_string(), bold(15.0), Color32::WHITE).size().x;
    let size = Vec2::new(size.x.max(text_w + 30.0), size.y);
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
        if !enabled { TEXT_DIM } else if luma(c1) + luma(c2) > 300.0 { INK } else { Color32::WHITE },
    );
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Simple monochrome line icons, drawn by the app so they look the same on every system
/// (emoji icons vary between fonts and some render as blobs at small sizes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
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

/// Draw `kind` centred on `c`, `size` points across (designed on a 24×24 grid).
pub fn icon(p: &egui::Painter, kind: Icon, c: Pos2, size: f32, col: Color32) {
    let u = size / 24.0;
    let pt = |x: f32, y: f32| c + Vec2::new(x * u, y * u);
    let st = Stroke::new((1.8 * u).max(1.2), col);
    let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pt(x0, y0), pt(x1, y1));
    let frame = |r: Rect, radius: f32| {
        p.rect_stroke(r, radius * u, st, StrokeKind::Middle);
    };
    let line = |pts: &[(f32, f32)]| {
        p.add(Shape::line(pts.iter().map(|(x, y)| pt(*x, *y)).collect(), st));
    };
    let arc = |r: f32, from: f32, to: f32| {
        let pts: Vec<Pos2> = (0..=12)
            .map(|k| {
                let a = (from + (to - from) * k as f32 / 12.0).to_radians();
                pt(-1.0, 0.0) + Vec2::new(a.cos(), a.sin()) * r * u
            })
            .collect();
        p.add(Shape::line(pts, st));
    };
    let dot = |x: f32, y: f32, r: f32| {
        p.circle_filled(pt(x, y), r * u, col);
    };
    match kind {
        Icon::Home => {
            line(&[(-10.0, 0.0), (0.0, -9.0), (10.0, 0.0)]);
            frame(rect(-6.5, -2.5, 6.5, 9.0), 1.5);
            frame(rect(-2.0, 3.0, 2.0, 9.0), 0.5);
        }
        Icon::Keyboard => {
            frame(rect(-11.0, -7.0, 11.0, 7.0), 2.5);
            for y in [-3.0, 0.5] {
                for k in 0..5 {
                    dot(-7.0 + k as f32 * 3.5, y, 1.1);
                }
            }
            line(&[(-5.0, 4.0), (5.0, 4.0)]);
        }
        Icon::Mouse => {
            frame(rect(-6.5, -10.0, 6.5, 10.0), 6.5);
            line(&[(-6.5, -2.0), (6.5, -2.0)]);
            line(&[(0.0, -10.0), (0.0, -2.0)]);
        }
        Icon::Gamepad => {
            frame(rect(-11.5, -6.5, 11.5, 6.5), 6.5);
            line(&[(-8.0, 0.0), (-3.0, 0.0)]);
            line(&[(-5.5, -2.5), (-5.5, 2.5)]);
            dot(4.5, -1.5, 1.4);
            dot(7.5, 1.5, 1.4);
        }
        Icon::Display => {
            frame(rect(-11.0, -8.0, 11.0, 5.0), 1.5);
            line(&[(0.0, 5.0), (0.0, 9.0)]);
            line(&[(-5.0, 9.0), (5.0, 9.0)]);
        }
        Icon::Audio => {
            let body: Vec<Pos2> =
                [(-11.0, -3.5), (-7.0, -3.5), (-2.0, -8.5), (-2.0, 8.5), (-7.0, 3.5), (-11.0, 3.5)].iter().map(|(x, y)| pt(*x, *y)).collect();
            p.add(Shape::closed_line(body, st));
            arc(5.5, -45.0, 45.0);
            arc(10.0, -45.0, 45.0);
        }
        Icon::Camera => {
            frame(rect(-11.0, -5.5, 11.0, 8.5), 2.5);
            line(&[(-5.0, -5.5), (-3.0, -8.5), (3.0, -8.5), (5.0, -5.5)]);
            p.circle_stroke(pt(0.0, 1.5), 4.2 * u, st);
            dot(7.5, -2.5, 1.0);
        }
        Icon::System => {
            // A chip with pins.
            frame(rect(-6.5, -6.5, 6.5, 6.5), 1.5);
            frame(rect(-2.5, -2.5, 2.5, 2.5), 0.5);
            for k in [-3.5f32, 0.0, 3.5] {
                line(&[(k, -6.5), (k, -10.0)]);
                line(&[(k, 6.5), (k, 10.0)]);
                line(&[(-6.5, k), (-10.0, k)]);
                line(&[(6.5, k), (10.0, k)]);
            }
        }
        Icon::Report => {
            frame(rect(-7.5, -8.5, 7.5, 10.5), 1.5);
            frame(rect(-3.5, -11.0, 3.5, -7.0), 1.0);
            line(&[(-4.0, -2.0), (4.0, -2.0)]);
            line(&[(-4.0, 2.0), (4.0, 2.0)]);
            line(&[(-4.0, 6.0), (1.0, 6.0)]);
        }
        Icon::Settings => {
            p.circle_stroke(c, 6.0 * u, st);
            p.circle_stroke(c, 2.3 * u, st);
            for k in 0..8 {
                let a = (k as f32 * 45.0).to_radians();
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * 6.0 * u, c + d * 9.5 * u], Stroke::new(st.width * 1.6, col));
            }
        }
    }
}

/// An on/off switch with a bold label and a dim hint underneath. Never takes keyboard focus.
/// Returns true when it was flipped.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str, hint: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(46.0, 26.0), Sense::CLICK);
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.clicked() {
            *on = !*on;
            changed = true;
        }
        let k = ui.ctx().animate_bool_with_time(resp.id.with("on"), *on, 0.15);
        let p = ui.painter();
        if k > 0.01 {
            hgradient(p, r, 13.0, alpha(ACCENT, k), alpha(SILVER, k));
        }
        p.rect_filled(r, 13.0, Color32::from_rgba_unmultiplied(40, 40, 44, (220.0 * (1.0 - k)) as u8));
        p.rect_stroke(r, 13.0, Stroke::new(1.0, alpha(LIGHT, 0.25)), StrokeKind::Inside);
        let knob = Pos2::new(lerp(r.left() + 13.0, r.right() - 13.0, k), r.center().y);
        p.circle_filled(knob, 10.0, lerp_color(LIGHT, INK, k));
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).font(bold(14.5)).color(Color32::WHITE));
            if !hint.is_empty() {
                ui.label(egui::RichText::new(hint).size(12.5).color(TEXT_DIM));
            }
        });
    });
    changed
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Segmented pill buttons for picking one option. Never takes keyboard focus (so key tests
/// can't trigger it). Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, id: &str, options: &[(T, &str, &str)], value: &mut T) -> bool {
    let font = bold(13.0);
    let pad = 14.0;
    let h = 28.0;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, label, _)| ui.painter().layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE).size().x + pad * 2.0)
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0;
    let (bar, _) = ui.allocate_exact_size(Vec2::new(total, h + 6.0), Sense::hover());
    let p = ui.painter().clone();
    p.rect_filled(bar, (h + 6.0) / 2.0, Color32::from_rgba_unmultiplied(10, 10, 12, 210));
    p.rect_stroke(bar, (h + 6.0) / 2.0, Stroke::new(1.0, alpha(LIGHT, 0.15)), StrokeKind::Inside);

    let mut changed = false;
    let mut x = bar.left() + 3.0;
    for (i, ((opt, label, hint), w)) in options.iter().zip(&widths).enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, bar.top() + 3.0), Vec2::new(*w, h));
        x += w;
        let resp = ui
            .interact(r, ui.scope_id().with((id, i)), Sense::CLICK)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(*hint);
        if resp.clicked() && *value != *opt {
            *value = *opt;
            changed = true;
        }
        let sel = *value == *opt;
        let on = ui.ctx().animate_bool_with_time(resp.id.with("sel"), sel, 0.18);
        let hover = ui.ctx().animate_bool_with_time(resp.id.with("hov"), resp.hovered() && !sel, 0.12);
        if on > 0.01 {
            hgradient(&p, r, h / 2.0, alpha(ACCENT, on), alpha(SILVER, on));
        } else if hover > 0.0 {
            p.rect_filled(r, h / 2.0, alpha(Color32::WHITE, 0.07 * hover));
        }
        let col = lerp_color(lerp_color(TEXT_DIM, TEXT, hover), INK, on);
        p.text(r.center(), Align2::CENTER_CENTER, *label, font.clone(), col);
    }
    changed
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
    /// The screentone picture and the window size / scale it was drawn for.
    tone: Option<(Rect, f32, egui::TextureHandle)>,
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
        let ppp = ctx.pixels_per_point();
        if self.tone.as_ref().is_none_or(|(r, s, _)| *r != screen || *s != ppp) {
            let img = screentone_image(screen, ppp);
            self.tone = Some((screen, ppp, ctx.load_texture("screentone", img, egui::TextureOptions::NEAREST)));
        }
        if let Some((_, _, tex)) = &self.tone {
            p.image(tex.id(), screen, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
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

/// Manga screentone: halftone dots fading out from two corners, drawn once into an image
/// (in physical pixels) so each frame draws one picture instead of ~35,000 vertices.
fn screentone_image(screen: Rect, ppp: f32) -> egui::ColorImage {
    let (w, h) = ((screen.width() * ppp).round().max(1.0) as usize, (screen.height() * ppp).round().max(1.0) as usize);
    let mut img = egui::ColorImage::filled([w, h], Color32::TRANSPARENT);
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
                // Anti-aliased round dot.
                let r = (0.4 + k * 2.6) * ppp;
                let (cx, cy) = ((pt.x - screen.left()) * ppp, (pt.y - screen.top()) * ppp);
                let (x0, x1) = ((cx - r - 1.0).floor().max(0.0) as usize, ((cx + r + 1.0).ceil() as usize).min(w));
                let (y0, y1) = ((cy - r - 1.0).floor().max(0.0) as usize, ((cy + r + 1.0).ceil() as usize).min(h));
                for py in y0..y1 {
                    for px in x0..x1 {
                        let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
                        let cover = (r - d + 0.5).clamp(0.0, 1.0);
                        if cover > 0.0 {
                            img.pixels[py * w + px] = Color32::from_white_alpha((0.07 * 255.0 * cover) as u8);
                        }
                    }
                }
            }
            x += spacing;
        }
        y += spacing * 0.87;
        row += 1;
    }
    img
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
    /// The message asked for during this frame. Several parts of a page may call `say`;
    /// the last one wins, and it's applied once per frame so the bubble never flickers.
    next: Option<String>,
    happy_until: f32,
    now: f32,
}

impl Mascot {
    pub fn new() -> Self {
        Self { msg: crate::i18n::t("Hi! Let's check your hardware~ ♪").into(), msg_since: 0.0, next: None, happy_until: 1.5, now: 0.0 }
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        self.next = Some(msg.into());
    }

    pub fn cheer(&mut self) {
        self.happy_until = self.now + 0.6;
    }

    /// Show the message chosen last frame; restart the typing animation only if it changed.
    fn apply_next(&mut self, now: f32) {
        if let Some(m) = self.next.take() {
            if m != self.msg {
                self.msg = m;
                self.msg_since = now;
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.now = ui.input(|i| i.time as f32);
        self.apply_next(self.now);
        let t = self.now;
        // Bubble sizes itself to the message and shrinks (or hides) when space is tight.
        let font = FontId::proportional(13.0);
        let full_w = ui.painter().layout_no_wrap(self.msg.clone(), font.clone(), Color32::WHITE).size().x;
        let avail = ui.available_width();
        let bubble_w = (full_w + 28.0).min(avail - 76.0).min(440.0);
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

#[cfg(test)]
mod tests {
    use super::Mascot;

    /// Where egui puts a glyph's baseline below the row top (same formula as epaint's layout).
    fn baseline(lead: [f32; 4], face: [f32; 4], offset: f32) -> f32 {
        let h = |m: [f32; 4]| m[0] - m[1] + m[2];
        face[0] + 0.5 * (h(lead) - h(face)) + offset
    }

    #[test]
    fn mixed_fonts_share_one_baseline() {
        // Roughly Yu Gothic, Microsoft YaHei, egui's Ubuntu and Noto Sans CJK.
        let yu = [1.07, -0.26, 0.0, 0.72];
        let others = [[1.06, -0.29, 0.0, 0.71], [0.93, -0.19, 0.03, 0.69], [1.16, -0.288, 0.0, 0.733]];
        let mut all = vec![Some(yu)];
        all.extend(others.iter().map(|m| Some(*m)));
        let off = super::baseline_offsets(yu, &all, true);
        let lead = baseline(yu, yu, off[0]);
        for (m, o) in others.iter().zip(&off[1..]) {
            assert!((baseline(yu, *m, *o) - lead).abs() < 1e-5);
        }
        // Noto leading its own family needs no shift.
        let noto = others[2];
        assert!(super::baseline_offsets(noto, &[Some(noto)], true)[0].abs() < 1e-5);
    }

    /// The Audio tab bug: the speaker test and the mic test both call `say` every frame.
    /// The bubble must settle on one message instead of restarting its typing each frame.
    #[test]
    fn two_speakers_per_frame_do_not_flicker() {
        let mut m = Mascot::new();
        for frame in 0..120 {
            m.say("Turn your volume down a bit first, ne~");
            m.say("Talk, clap or snap your fingers~ ♪");
            m.apply_next(frame as f32 / 60.0);
        }
        assert_eq!(m.msg, "Talk, clap or snap your fingers~ ♪");
        // Typing started on the first frame and was never restarted.
        assert_eq!(m.msg_since, 0.0);
    }

    #[test]
    fn a_new_message_restarts_typing_once() {
        let mut m = Mascot::new();
        m.say("A");
        m.apply_next(1.0);
        m.say("B");
        m.apply_next(2.0);
        m.say("B");
        m.apply_next(3.0);
        assert_eq!(m.msg, "B");
        assert_eq!(m.msg_since, 2.0);
    }
}
