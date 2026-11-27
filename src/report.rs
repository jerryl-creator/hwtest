//! Report tab: a summary of every test, which can be copied or saved as text or an image.

use crate::i18n::t;
use crate::theme::{self, alpha, Mascot};
use eframe::egui::{self, Color32, Vec2};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    NotTested,
    InProgress,
    Pass,
    Issue,
}

impl Status {
    fn badge(self) -> &'static str {
        t(match self {
            Status::NotTested => "– Not tested",
            Status::InProgress => "… In progress",
            Status::Pass => "✔ Pass",
            Status::Issue => "⚠ Issue found",
        })
    }
}

pub struct Section {
    pub title: &'static str,
    pub jp: &'static str,
    pub status: Status,
    pub lines: Vec<String>,
}

/// Info about this computer for the report header.
pub struct Header {
    pub renderer: String,
}

pub fn to_text(header: &Header, sections: &[Section]) -> String {
    let mut out = String::new();
    out.push_str(&tr!("Nyanspect {0} hardware report", env!("CARGO_PKG_VERSION")));
    out.push('\n');
    out.push_str(&format!("{} {}\n", t("Date:"), now_utc()));
    out.push_str(&format!("{} {} ({})\n", t("System:"), os_name(), std::env::consts::ARCH));
    out.push_str(&format!("{} {}\n", t("Graphics:"), header.renderer));
    out.push_str(&"=".repeat(48));
    out.push('\n');
    for s in sections {
        out.push_str(&format!("\n{}  {}\n", t(s.title), s.status.badge()));
        for l in &s.lines {
            out.push_str(&format!("  - {l}\n"));
        }
    }
    out.push_str(&format!("\n{} https://github.com/jerryl-creator/nyanspect\n", t("Made with Nyanspect:")));
    out
}

fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "linux" => "Linux",
        "macos" => "macOS",
        other => other,
    }
}

/// "YYYY-MM-DD HH:MM UTC" without pulling in a date library.
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rem / 3600, (rem % 3600) / 60)
}

fn stamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, (rem % 3600) / 60, rem % 60)
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Where saved reports go: Downloads, else Documents, else the home folder.
fn save_dir() -> PathBuf {
    dirs::download_dir().or_else(dirs::document_dir).or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Default)]
pub struct ReportTab {
    /// Feedback after copy/save, and when it happened.
    message: Option<(String, f64)>,
    /// Set when a quick check finishes: the steps that were skipped (English names).
    pub quick_done: Option<Vec<&'static str>>,
}

impl ReportTab {
    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool, header: &Header, sections: &[Section]) {
        let now = ui.input(|i| i.time);
        let count = |st: Status| sections.iter().filter(|s| s.status == st).count();
        let (pass, issue, tested) = (count(Status::Pass), count(Status::Issue), sections.len() - count(Status::NotTested));
        if tested == 0 {
            mascot.say(t("Run some tests and your report card appears here~"));
        } else if issue > 0 {
            mascot.say(tr!("{0} thing(s) need a look… see below (；ω；)", issue));
        } else {
            mascot.say(t("Here's your hardware report card~ ♪"));
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Report", "レポート", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = Vec2::new(150.0, 34.0);
                    if theme::fancy_button(ui, t("Save as image"), b, theme::SILVER, theme::STEEL, true).clicked() {
                        let path = save_dir().join(format!("nyanspect-report-{}.png", stamp()));
                        let shown = path.display().to_string();
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                            let [w, h] = img.size;
                            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, img.as_raw().to_vec()) {
                                if let Err(e) = buf.save(&path) {
                                    eprintln!("could not save report image: {e}");
                                }
                            }
                        })));
                        self.message = Some((tr!("Saved image to {0}", shown), now));
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("Save as text"), b, theme::LIGHT, theme::SILVER, true).clicked() {
                        let path = save_dir().join(format!("nyanspect-report-{}.txt", stamp()));
                        self.message = Some((
                            match std::fs::write(&path, to_text(header, sections)) {
                                Ok(()) => tr!("Saved to {0}", path.display()),
                                Err(e) => tr!("Couldn't save: {0}", e),
                            },
                            now,
                        ));
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("Copy"), Vec2::new(100.0, 34.0), theme::ACCENT, theme::SILVER, true).clicked() {
                        ui.ctx().copy_text(to_text(header, sections));
                        self.message = Some((t("Copied! Paste it anywhere.").to_string(), now));
                    }
                });
            });
            ui.label(
                egui::RichText::new(format!(
                    "{}   ·   {} · {} · {}",
                    tr!("{0} passed · {1} with issues · {2} not tested", pass, issue, count(Status::NotTested)),
                    os_name(),
                    header.renderer,
                    now_utc()
                ))
                .color(theme::TEXT_DIM),
            );
            if let Some(skipped) = &self.quick_done {
                let text = if skipped.is_empty() {
                    t("✔ Quick check finished: every step was checked.").to_string()
                } else {
                    tr!("✔ Quick check finished. Skipped: {0}", skipped.iter().map(|s| t(s)).collect::<Vec<_>>().join(", "))
                };
                ui.label(egui::RichText::new(text).font(theme::bold(13.5)).color(Color32::WHITE));
            }
            if let Some((msg, at)) = &self.message {
                if now - at < 6.0 {
                    ui.label(egui::RichText::new(msg).font(theme::bold(13.0)).color(Color32::WHITE));
                }
            }
        });
        ui.add_space(12.0);

        // Two columns of section cards.
        ui.columns(2, |cols| {
            for (i, s) in sections.iter().enumerate() {
                let ui = &mut cols[i % 2];
                theme::card().show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        theme::card_title(ui, s.title, s.jp, cjk);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (fill, text) = match s.status {
                                Status::Pass => (Color32::WHITE, theme::INK),
                                Status::Issue => (theme::GLINT, theme::INK),
                                Status::InProgress => (alpha(theme::SILVER, 0.35), Color32::WHITE),
                                Status::NotTested => (alpha(theme::SILVER, 0.15), theme::TEXT_DIM),
                            };
                            egui::Frame::new().fill(fill).corner_radius(12).inner_margin(egui::Margin::symmetric(10, 3)).show(ui, |ui| {
                                ui.label(egui::RichText::new(s.status.badge()).font(theme::bold(13.0)).color(text));
                            });
                        });
                    });
                    for l in &s.lines {
                        ui.label(egui::RichText::new(format!("• {l}")).color(theme::TEXT));
                    }
                });
                ui.add_space(12.0);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(civil_from_days(20_783), (2026, 11, 26));
    }

    #[test]
    fn text_report_lists_every_section() {
        let sections = vec![
            Section { title: "Keyboard", jp: "", status: Status::Pass, lines: vec!["104 / 104 keys".into()] },
            Section { title: "Mouse", jp: "", status: Status::NotTested, lines: vec![] },
        ];
        let t = to_text(&Header { renderer: "Test GPU".into() }, &sections);
        assert!(t.contains("Keyboard") && t.contains("✔ Pass") && t.contains("104 / 104 keys"));
        assert!(t.contains("Mouse") && t.contains("Not tested"));
        assert!(t.contains("Graphics: Test GPU"));
    }
}
