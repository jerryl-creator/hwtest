//! Manga-style sound-effect pop-ups ("ドン!", "バン!") that burst out of pressed keys.

use crate::theme::{self, alpha, INK};
use eframe::egui::{self, epaint::{Mesh, TextShape}, Align2, Color32, Key, Pos2, Rect, Shape, Stroke, Vec2};
use std::f32::consts::TAU;

/// (Japanese, romaji)
type Word = (&'static str, &'static str);

const POOL: &[Word] = &[
    ("バン!", "BAN!"),
    ("ドン!", "DON!"),
    ("ゴゴゴ", "GOGOGO"),
    ("キラッ☆", "KIRA!"),
    ("ポチッ", "POCHI"),
    ("カチッ!", "KACHI!"),
    ("すごい!", "SUGOI!"),
    ("やった!", "YATTA!"),
    ("かわいい!", "KAWAII!"),
    ("なに?!", "NANI?!"),
    ("ズドン!", "ZUDON!"),
    ("バキッ!", "BAKI!"),
    ("ニャー!", "NYAA!"),
    ("えいっ!", "EI!"),
    ("ワクワク", "WAKU WAKU"),
    ("ピカッ!", "PIKA!"),
];

/// Special words for special keys. Returns (word, is_big).
fn word_for(key: Key, roll: f32) -> (Word, bool) {
    use Key::*;
    let special: Option<Word> = match key {
        Space => Some(("ドーン!!", "DOOON!!")),
        Enter | F35 => Some(("決定!", "KETTEI!")),
        Backspace | Delete => Some(("消えろ!", "KIERO!")),
        Escape => Some(("逃げろ!", "NIGERO!")),
        ShiftLeft | ShiftRight => Some(("シュッ!", "SHU!")),
        ControlLeft | ControlRight => Some(("ガシッ!", "GASHI!")),
        AltLeft | AltRight => Some(("ヒュン!", "HYUN!")),
        SuperLeft | SuperRight => Some(("神!", "KAMI!")),
        CapsLock => Some(("叫べ!", "SAKEBE!")),
        Tab => Some(("スッ", "SU")),
        ArrowUp | ArrowDown | ArrowLeft | ArrowRight => Some(("ビュン!", "BYUN!")),
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => Some(("必殺技!", "HISSATSU!")),
        _ => None,
    };
    match special {
        Some(w) => (w, matches!(key, Space | Enter | F35)),
        None => (POOL[((roll * POOL.len() as f32) as usize).min(POOL.len() - 1)], false),
    }
}

struct Pop {
    word: Word,
    center: Pos2,
    drift: Vec2,
    angle: f32,
    age: f32,
    big: bool,
    /// Per-spike jitter so each burst has its own jagged shape.
    jitter: [f32; 28],
}

pub struct Sfx {
    pops: Vec<Pop>,
    rng: u32,
    pub enabled: bool,
}

impl Default for Sfx {
    fn default() -> Self {
        Self { pops: vec![], rng: 0x5EED_1234, enabled: true }
    }
}

const LIFE: f32 = 1.15;

impl Sfx {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng % 10_000) as f32 / 10_000.0
    }

    /// Pop a word out of the key at `cap`.
    pub fn spawn(&mut self, key: Key, cap: Rect) {
        if !self.enabled {
            return;
        }
        let roll = self.rand();
        let (word, big) = word_for(key, roll);
        let mut jitter = [0.0; 28];
        for j in jitter.iter_mut() {
            *j = self.rand();
        }
        let center = cap.center_top() + Vec2::new((self.rand() - 0.5) * 40.0, -28.0 - self.rand() * 18.0);
        let pop = Pop {
            word,
            center,
            drift: Vec2::new((self.rand() - 0.5) * 50.0, -50.0 - self.rand() * 30.0),
            angle: (self.rand() - 0.5) * 0.5, // about ±14°
            age: 0.0,
            big,
            jitter,
        };
        self.pops.push(pop);
        if self.pops.len() > 14 {
            self.pops.remove(0);
        }
    }

    pub fn paint(&mut self, ctx: &egui::Context, dt: f32, cjk: bool) {
        if self.pops.is_empty() {
            return;
        }
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("sfx")));
        let screen = ctx.content_rect();
        for pop in &mut self.pops {
            pop.age += dt;
        }
        self.pops.retain(|p| p.age < LIFE);

        for pop in &self.pops {
            let t = pop.age;
            // Pop in with overshoot, hold, then fade out.
            let scale = if t < 0.16 { ease_out_back(t / 0.16) } else { 1.0 };
            let fade = ((LIFE - t) / 0.35).clamp(0.0, 1.0);
            let (jp, romaji) = pop.word;
            let text = if cjk { jp } else { romaji };
            let size = if pop.big { 40.0 } else { 30.0 } * scale.max(0.05);

            let galley = painter.layout_no_wrap(text.to_string(), theme::bold(size), INK);
            let sub = cjk.then(|| painter.layout_no_wrap(romaji.to_string(), theme::bold(size * 0.36), INK));
            let sub_h = sub.as_ref().map_or(0.0, |g| g.size().y);
            let half = Vec2::new(galley.size().x.max(sub.as_ref().map_or(0.0, |g| g.size().x)) / 2.0 + 16.0 * scale, (galley.size().y + sub_h) / 2.0 + 12.0 * scale);

            // Keep the burst on screen.
            let mut c = pop.center + pop.drift * t;
            c.x = c.x.clamp(screen.left() + half.x + 8.0, screen.right() - half.x - 8.0);
            c.y = c.y.clamp(screen.top() + half.y + 8.0, screen.bottom() - half.y - 8.0);

            // Speed / focus lines radiating out (first half-second).
            if t < 0.5 {
                let k = 1.0 - t / 0.5;
                for i in 0..14 {
                    let a = i as f32 / 14.0 * TAU + pop.jitter[i] * 0.3;
                    let dir = Vec2::angled(a);
                    let r0 = half.x.max(half.y) * 1.05 + t * 90.0;
                    let r1 = r0 + 14.0 + pop.jitter[i + 14] * 22.0;
                    painter.line_segment([c + dir * r0, c + dir * r1], Stroke::new(2.5 * k, alpha(Color32::WHITE, k * fade)));
                }
            }

            // Jagged white burst with an ink outline.
            let (sa, ca) = pop.angle.sin_cos();
            let n = 14;
            let pts: Vec<Pos2> = (0..n * 2)
                .map(|i| {
                    let a = i as f32 / (n * 2) as f32 * TAU;
                    let spike = if i % 2 == 0 { 1.18 + pop.jitter[i] * 0.22 } else { 0.92 };
                    let local = Vec2::new(a.cos() * half.x * spike, a.sin() * half.y * spike);
                    c + Vec2::new(local.x * ca - local.y * sa, local.x * sa + local.y * ca)
                })
                .collect();
            // drop shadow
            let shadow: Vec<Pos2> = pts.iter().map(|p| *p + Vec2::new(4.0, 5.0)).collect();
            fan(&painter, c + Vec2::new(4.0, 5.0), &shadow, alpha(Color32::BLACK, 0.55 * fade));
            fan(&painter, c, &pts, alpha(Color32::WHITE, fade));
            painter.add(Shape::closed_line(pts, Stroke::new(2.5, alpha(INK, fade))));

            // The word, plus romaji underneath when showing Japanese. Both rotate with the burst:
            // a local offset (0, d) becomes (-d·sin, d·cos) on screen.
            let gh = galley.size().y;
            let along = |d: f32| Vec2::new(-d * sa, d * ca);
            let main_center = c + along(-sub_h / 2.0);
            painter.add(
                TextShape::new(main_center - galley.size() / 2.0, galley, INK)
                    .with_angle_and_anchor(pop.angle, Align2::CENTER_CENTER)
                    .with_opacity_factor(fade),
            );
            if let Some(sub) = sub {
                let sub_center = c + along(gh / 2.0 - 2.0);
                painter.add(
                    TextShape::new(sub_center - sub.size() / 2.0, sub, INK)
                        .with_angle_and_anchor(pop.angle, Align2::CENTER_CENTER)
                        .with_opacity_factor(fade * 0.75),
                );
            }
        }
    }
}

fn ease_out_back(x: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (x - 1.0).powi(3) + c1 * (x - 1.0).powi(2)
}

/// Fill a star-shaped polygon by fanning triangles from its centre.
fn fan(painter: &egui::Painter, center: Pos2, pts: &[Pos2], color: Color32) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for p in pts {
        mesh.colored_vertex(*p, color);
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    painter.add(Shape::mesh(mesh));
}
