//! On-screen keyboard layouts: three sizes (full, TKL, 75% / laptop) in ANSI (US) or ISO (UK/EU) style.

use eframe::egui::Key;

/// One keycap: label, key, position and size in key units.
pub struct Cap {
    pub label: &'static str,
    pub key: Key,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Size {
    /// 104/105 keys, with numpad
    Full,
    /// Tenkeyless: 87/88 keys, no numpad
    Tkl,
    /// 75% / laptop: compact, no numpad, nav keys in a column
    Compact,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Style {
    /// US: wide Enter, long left Shift
    Ansi,
    /// UK/EU: tall Enter, # key, extra \| key next to a short left Shift
    Iso,
}

impl Size {
    pub const ALL: [Size; 3] = [Size::Full, Size::Tkl, Size::Compact];
    pub fn label(self) -> &'static str {
        match self {
            Size::Full => "Full",
            Size::Tkl => "TKL",
            Size::Compact => "75% / Laptop",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Size::Full => "Full-size keyboard with number pad",
            Size::Tkl => "Tenkeyless: full-size without the number pad",
            Size::Compact => "Compact 75% keyboards and most laptops (Fn can't be detected by apps)",
        }
    }
}

impl Style {
    pub const ALL: [Style; 2] = [Style::Ansi, Style::Iso];
    pub fn label(self) -> &'static str {
        match self {
            Style::Ansi => "US (ANSI)",
            Style::Iso => "UK/EU (ISO)",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Style::Ansi => "Wide one-row Enter key and a long left Shift",
            Style::Iso => "Tall two-row Enter key and an extra key next to a short left Shift",
        }
    }
}

pub struct Layout {
    pub caps: Vec<Cap>,
    /// Size in key units.
    pub w: f32,
    pub h: f32,
}

// Numpad keys arrive as F20..F35 thanks to the patch in vendor/egui-winit.
pub const NP0: Key = Key::F20;
pub const NP1: Key = Key::F21;
pub const NP2: Key = Key::F22;
pub const NP3: Key = Key::F23;
pub const NP4: Key = Key::F24;
pub const NP5: Key = Key::F25;
pub const NP6: Key = Key::F26;
pub const NP7: Key = Key::F27;
pub const NP8: Key = Key::F28;
pub const NP9: Key = Key::F29;
pub const NP_DOT: Key = Key::F30;
pub const NP_ADD: Key = Key::F31;
pub const NP_SUB: Key = Key::F32;
pub const NP_MUL: Key = Key::F33;
pub const NP_DIV: Key = Key::F34;
pub const NP_ENTER: Key = Key::F35;

/// Builds a row of caps left to right.
struct Row<'a> {
    caps: &'a mut Vec<Cap>,
    x: f32,
    y: f32,
}

impl Row<'_> {
    fn key(&mut self, label: &'static str, key: Key, w: f32) -> &mut Self {
        self.caps.push(Cap { label, key, x: self.x, y: self.y, w, h: 1.0 });
        self.x += w;
        self
    }
    fn k(&mut self, label: &'static str, key: Key) -> &mut Self {
        self.key(label, key, 1.0)
    }
    /// A key two rows tall (numpad + and Enter, ISO Enter).
    fn tall(&mut self, label: &'static str, key: Key, w: f32) -> &mut Self {
        self.caps.push(Cap { label, key, x: self.x, y: self.y, w, h: 2.0 });
        self.x += w;
        self
    }
    fn gap(&mut self, w: f32) -> &mut Self {
        self.x += w;
        self
    }
    fn at(&mut self, x: f32) -> &mut Self {
        self.x = x;
        self
    }
}

fn row(caps: &mut Vec<Cap>, y: f32) -> Row<'_> {
    Row { caps, x: 0.0, y }
}

pub fn build(size: Size, style: Style) -> Layout {
    use Key::*;
    let mut c = Vec::new();
    let compact = size == Size::Compact;
    let nav = 15.25; // navigation cluster (Full / TKL)
    let pad = 18.5; // numpad (Full)
    let col = 15.0; // right-hand column (75%)

    // Function row. Full/TKL leave half a row of space below it; 75% doesn't.
    if compact {
        row(&mut c, 0.0).k("Esc", Escape)
            .k("F1", F1).k("F2", F2).k("F3", F3).k("F4", F4).k("F5", F5).k("F6", F6)
            .k("F7", F7).k("F8", F8).k("F9", F9).k("F10", F10).k("F11", F11).k("F12", F12)
            .k("PrtSc", PrintScreen).k("Ins", Insert).k("Del", Delete);
    } else {
        row(&mut c, 0.0).k("Esc", Escape).gap(1.0)
            .k("F1", F1).k("F2", F2).k("F3", F3).k("F4", F4).gap(0.5)
            .k("F5", F5).k("F6", F6).k("F7", F7).k("F8", F8).gap(0.5)
            .k("F9", F9).k("F10", F10).k("F11", F11).k("F12", F12)
            .at(nav).k("PrtSc", PrintScreen).k("ScrLk", ScrollLock).k("Pause", Pause);
    }
    let y = if compact { 1.0 } else { 1.5 };

    // Number row (same in every layout).
    row(&mut c, y).k("`", Backtick).k("1", Num1).k("2", Num2).k("3", Num3).k("4", Num4).k("5", Num5)
        .k("6", Num6).k("7", Num7).k("8", Num8).k("9", Num9).k("0", Num0).k("-", Minus).k("=", Equals)
        .key("Backspace", Backspace, 2.0);

    // Letter rows: ANSI and ISO differ around Enter and left Shift.
    match style {
        Style::Ansi => {
            row(&mut c, y + 1.0).key("Tab", Tab, 1.5).k("Q", Q).k("W", W).k("E", E).k("R", R).k("T", T).k("Y", Y)
                .k("U", U).k("I", I).k("O", O).k("P", P).k("[", OpenBracket).k("]", CloseBracket)
                .key("\\", Backslash, 1.5);
            row(&mut c, y + 2.0).key("Caps Lock", CapsLock, 1.75).k("A", A).k("S", S).k("D", D).k("F", F).k("G", G)
                .k("H", H).k("J", J).k("K", K).k("L", L).k(";", Semicolon).k("'", Quote)
                .key("Enter", Enter, 2.25);
            row(&mut c, y + 3.0).key("Shift", ShiftLeft, 2.25);
        }
        Style::Iso => {
            row(&mut c, y + 1.0).key("Tab", Tab, 1.5).k("Q", Q).k("W", W).k("E", E).k("R", R).k("T", T).k("Y", Y)
                .k("U", U).k("I", I).k("O", O).k("P", P).k("[", OpenBracket).k("]", CloseBracket)
                .gap(0.25).tall("Enter", Enter, 1.25);
            // On ISO boards the key left of Enter reports the same physical code as ANSI's backslash.
            row(&mut c, y + 2.0).key("Caps Lock", CapsLock, 1.75).k("A", A).k("S", S).k("D", D).k("F", F).k("G", G)
                .k("H", H).k("J", J).k("K", K).k("L", L).k(";", Semicolon).k("'", Quote)
                .k("#", Backslash);
            row(&mut c, y + 3.0).key("Shift", ShiftLeft, 1.25).k("\\", IntlBackslash);
        }
    }
    // Rest of the Shift row; on 75% the right Shift is shorter to fit the arrow key.
    {
        let start = c.last().map(|k| k.x + k.w).unwrap_or(0.0);
        let mut r = Row { caps: &mut c, x: start, y: y + 3.0 };
        r.k("Z", Z).k("X", X).k("C", C).k("V", V).k("B", B).k("N", N).k("M", M)
            .k(",", Comma).k(".", Period).k("/", Slash)
            .key("Shift", ShiftRight, if compact { 1.75 } else { 2.75 });
    }

    // Bottom row.
    if compact {
        row(&mut c, y + 4.0).key("Ctrl", ControlLeft, 1.25).key("Win", SuperLeft, 1.25).key("Alt", AltLeft, 1.25)
            .key("", Space, 6.25)
            .k("Alt", AltRight).k("Ctrl", ControlRight)
            .at(13.0).k("←", ArrowLeft).k("↓", ArrowDown).k("→", ArrowRight);
    } else {
        row(&mut c, y + 4.0).key("Ctrl", ControlLeft, 1.25).key("Win", SuperLeft, 1.25).key("Alt", AltLeft, 1.25)
            .key("", Space, 6.25)
            .key("Alt", AltRight, 1.25).key("Win", SuperRight, 1.25).key("Menu", Menu, 1.25)
            .key("Ctrl", ControlRight, 1.25);
    }

    // Navigation keys and arrows.
    if compact {
        row(&mut c, y).at(col).k("Home", Home);
        row(&mut c, y + 1.0).at(col).k("PgUp", PageUp);
        row(&mut c, y + 2.0).at(col).k("PgDn", PageDown);
        row(&mut c, y + 3.0).at(14.0).k("↑", ArrowUp).k("End", End);
    } else {
        row(&mut c, y).at(nav).k("Ins", Insert).k("Home", Home).k("PgUp", PageUp);
        row(&mut c, y + 1.0).at(nav).k("Del", Delete).k("End", End).k("PgDn", PageDown);
        row(&mut c, y + 3.0).at(nav + 1.0).k("↑", ArrowUp);
        row(&mut c, y + 4.0).at(nav).k("←", ArrowLeft).k("↓", ArrowDown).k("→", ArrowRight);
    }

    // Number pad.
    if size == Size::Full {
        row(&mut c, y).at(pad).k("Num", NumLock).k("/", NP_DIV).k("*", NP_MUL).k("−", NP_SUB);
        row(&mut c, y + 1.0).at(pad).k("7", NP7).k("8", NP8).k("9", NP9).tall("+", NP_ADD, 1.0);
        row(&mut c, y + 2.0).at(pad).k("4", NP4).k("5", NP5).k("6", NP6);
        row(&mut c, y + 3.0).at(pad).k("1", NP1).k("2", NP2).k("3", NP3).tall("Enter", NP_ENTER, 1.0);
        row(&mut c, y + 4.0).at(pad).key("0", NP0, 2.0).k(".", NP_DOT);
    }

    let w = match size {
        Size::Full => 22.5,
        Size::Tkl => 18.25,
        Size::Compact => 16.0,
    };
    Layout { caps: c, w, h: y + 5.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn count(size: Size, style: Style) -> usize {
        build(size, style).caps.len()
    }

    #[test]
    fn key_counts_match_real_keyboards() {
        assert_eq!(count(Size::Full, Style::Ansi), 104);
        assert_eq!(count(Size::Full, Style::Iso), 105);
        assert_eq!(count(Size::Tkl, Style::Ansi), 87);
        assert_eq!(count(Size::Tkl, Style::Iso), 88);
        assert_eq!(count(Size::Compact, Style::Ansi), 83);
        assert_eq!(count(Size::Compact, Style::Iso), 84);
    }

    #[test]
    fn no_duplicate_keys_and_no_overlaps() {
        for size in Size::ALL {
            for style in Style::ALL {
                let l = build(size, style);
                let mut seen = HashSet::new();
                for cap in &l.caps {
                    assert!(seen.insert(cap.key), "{size:?}/{style:?}: {:?} appears twice", cap.key);
                    assert!(cap.x + cap.w <= l.w + 1e-3 && cap.y + cap.h <= l.h + 1e-3, "{size:?}/{style:?}: {} is outside", cap.label);
                }
                for (i, a) in l.caps.iter().enumerate() {
                    for b in &l.caps[i + 1..] {
                        let overlap = a.x < b.x + b.w - 1e-3 && b.x < a.x + a.w - 1e-3
                            && a.y < b.y + b.h - 1e-3 && b.y < a.y + a.h - 1e-3;
                        assert!(!overlap, "{size:?}/{style:?}: {} overlaps {}", a.label, b.label);
                    }
                }
            }
        }
    }
}
