//! Interface languages: English, Simplified Chinese, Malay and Japanese.
//!
//! Text in the code stays in English and is looked up at draw time:
//! `t("Reset")` for plain text, `tr!("{0} / {1} keys", done, total)` for text with values
//! (numbered so a translation can reorder them). The tables live in `table.rs`; a test
//! checks that every `t(..)` / `tr!(..)` in the source has a row there.

mod table;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    En,
    Zh,
    Ms,
    Ja,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::En, Lang::Zh, Lang::Ms, Lang::Ja];

    /// The language's own name, for the picker.
    pub fn native(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "简体中文",
            Lang::Ms => "Bahasa Melayu",
            Lang::Ja => "日本語",
        }
    }

    /// Short tag for the header button.
    pub fn short(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Zh => "中文",
            Lang::Ms => "BM",
            Lang::Ja => "日本語",
        }
    }

    /// Guess from a locale like "zh-CN", "ms_MY.UTF-8" or "ja-JP".
    pub fn from_locale(loc: &str) -> Lang {
        let l = loc.to_ascii_lowercase();
        if l.starts_with("zh") {
            Lang::Zh
        } else if l.starts_with("ms") {
            Lang::Ms
        } else if l.starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }

    /// The system's language, used until the user picks one.
    pub fn system() -> Lang {
        sys_locale::get_locale().map_or(Lang::En, |l| Lang::from_locale(&l))
    }

    fn column(self) -> usize {
        self as usize
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::Zh,
        2 => Lang::Ms,
        3 => Lang::Ja,
        _ => Lang::En,
    }
}

fn index() -> &'static HashMap<&'static str, &'static [&'static str; 4]> {
    static INDEX: OnceLock<HashMap<&'static str, &'static [&'static str; 4]>> = OnceLock::new();
    INDEX.get_or_init(|| table::ROWS.iter().flat_map(|part| part.iter()).map(|row| (row[0], row)).collect())
}

/// Translate English UI text into the current language (English if there's no row).
pub fn t(en: &str) -> &str {
    let l = lang();
    if l == Lang::En {
        return en;
    }
    index().get(en).map_or(en, |row| row[l.column()])
}

/// Translate a word that needs context, e.g. `tc("speaker", "Left")` (a speaker channel)
/// versus `t("Left")` (a mouse button). Its row is keyed "speaker|Left".
pub fn tc<'a>(context: &str, en: &'a str) -> &'a str {
    let l = lang();
    if l == Lang::En {
        return en;
    }
    index().get(format!("{context}|{en}").as_str()).map_or_else(|| t(en), |row| row[l.column()])
}

/// Fill `{0}`, `{1}`… in a translated template.
pub fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), &a.to_string());
    }
    out
}

/// `tr!("{0} / {1} keys", done, total)`: translate, then fill in the values.
#[macro_export]
macro_rules! tr {
    ($s:literal $(, $a:expr)* $(,)?) => {
        $crate::i18n::fill($crate::i18n::t($s), &[$(&$a as &dyn std::fmt::Display),*])
    };
}

/// The small second line under titles and tabs: Japanese for English and Malay (the app's
/// anime style), English for Chinese and Japanese (whose main line is already CJK).
pub fn subtitle<'a>(en: &'a str, jp: &'a str) -> &'a str {
    match lang() {
        Lang::En | Lang::Ms => jp,
        Lang::Zh | Lang::Ja => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = (0..10).map(|i| format!("{{{i}}}")).filter(|p| s.contains(p.as_str())).collect();
        v.sort();
        v
    }

    #[test]
    fn locales_map_to_languages() {
        assert_eq!(Lang::from_locale("zh-CN"), Lang::Zh);
        assert_eq!(Lang::from_locale("ms_MY.UTF-8"), Lang::Ms);
        assert_eq!(Lang::from_locale("ja-JP"), Lang::Ja);
        assert_eq!(Lang::from_locale("en-GB"), Lang::En);
        assert_eq!(Lang::from_locale("de-DE"), Lang::En);
    }

    #[test]
    fn every_row_is_complete_and_keeps_its_placeholders() {
        let mut seen = std::collections::HashSet::new();
        for row in table::ROWS.iter().flat_map(|p| p.iter()) {
            assert!(seen.insert(row[0]), "duplicate row: {:?}", row[0]);
            for (i, cell) in row.iter().enumerate() {
                assert!(!cell.trim().is_empty(), "empty cell {i} in {:?}", row[0]);
                assert_eq!(placeholders(cell), placeholders(row[0]), "placeholders differ in column {i} of {:?}", row[0]);
            }
        }
    }

    #[test]
    fn filling_values_in_any_order() {
        assert_eq!(fill("{1} of {0}", &[&3, &"keys"]), "keys of 3");
    }

    /// Every `t("…")` and `tr!("…")` in the source must have a row.
    #[test]
    fn every_ui_string_has_a_translation() {
        let mut missing = Vec::new();
        for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            for key in ui_strings(&src) {
                if !index().contains_key(key.as_str()) {
                    missing.push(format!("{}: {key:?}", path.file_name().unwrap().to_string_lossy()));
                }
            }
        }
        assert!(missing.is_empty(), "untranslated strings:\n{}", missing.join("\n"));
    }

    /// String literals passed to `t(` or `tr!(` (simple escapes only).
    fn ui_strings(src: &str) -> Vec<String> {
        // `t(\n    "…")` counts too: drop whitespace between "(" and a string.
        let mut flat = String::with_capacity(src.len());
        let mut chars = src.chars().peekable();
        while let Some(c) = chars.next() {
            flat.push(c);
            if c == '(' {
                let mut ws = String::new();
                while let Some(&n) = chars.peek() {
                    if n.is_whitespace() {
                        ws.push(n);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() != Some(&'"') {
                    flat.push_str(&ws);
                }
            }
        }
        let src = flat.as_str();
        let mut out = Vec::new();
        for marker in ["t(\"", "tr!(\"", "card_title(ui, \"", "title: \""] {
            let mut rest = src;
            while let Some(i) = rest.find(marker) {
                // Skip identifiers that merely end in "t(", like `format!(` or `set(`.
                let before = rest[..i].chars().last();
                let start = i + marker.len();
                rest = &rest[start..];
                if marker == "t(\"" && before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let mut s = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some(o) => s.push(o),
                            None => {}
                        },
                        _ => s.push(c),
                    }
                }
                out.push(s);
            }
        }
        out
    }
}
