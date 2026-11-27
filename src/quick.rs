//! Quick check: a guided walk through every test, one step at a time, ending at the report.
//!
//! It never blocks: each step shows what to do and its live result (the same status the
//! report uses), and Next is always available. Steps left untested count as skipped.

use crate::i18n::t;
use crate::report::Status;
use crate::theme::{self, alpha};
use eframe::egui::{self, Align2, Color32, Pos2, Sense, Stroke, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    System,
    Keyboard,
    Mouse,
    Display,
    Speakers,
    Microphone,
    Camera,
    Gamepad,
}

pub const STEPS: [Step; 8] = [Step::System, Step::Keyboard, Step::Mouse, Step::Display, Step::Speakers, Step::Microphone, Step::Camera, Step::Gamepad];

impl Step {
    pub fn name(self) -> &'static str {
        match self {
            Step::System => "System",
            Step::Keyboard => "Keyboard",
            Step::Mouse => "Mouse",
            Step::Display => "Display",
            Step::Speakers => "Speakers",
            Step::Microphone => "Microphone",
            Step::Camera => "Camera",
            Step::Gamepad => "Gamepad",
        }
    }

    fn how(self) -> &'static str {
        match self {
            Step::System => "Check the model, serial number and battery health, then press Next.",
            Step::Keyboard => "Press every key once: each one lights up when it works. Press Next when you're done.",
            Step::Mouse => "Click each mouse button, scroll up and down, and double-click in the box.",
            Step::Display => "Press “Start fullscreen test”, look for odd dots on every colour, then press Esc.",
            Step::Speakers => "Click each speaker circle and answer Yes or No under it.",
            Step::Microphone => "Press Start mic and talk, then Record 3 s & play back and say whether it sounded clear.",
            Step::Camera => "Press Start camera and check that the picture looks right.",
            Step::Gamepad => "Press every button and roll both sticks all the way around.",
        }
    }
}

/// What happened to a step when the user moved on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Pending,
    Checked,
    Skipped,
}

/// What the app should do after the guide bar was drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    None,
    /// Show this step's tab.
    Goto(Step),
    /// All steps done: show the report.
    Finish,
    Exit,
}

pub struct QuickCheck {
    pub current: usize,
    pub outcomes: [Outcome; STEPS.len()],
}

impl QuickCheck {
    pub fn new() -> Self {
        Self { current: 0, outcomes: [Outcome::Pending; STEPS.len()] }
    }

    pub fn step(&self) -> Step {
        STEPS[self.current]
    }

    /// Leave the current step (recording whether it was tested) and go to the next one.
    fn advance(&mut self, status: Status, skip: bool) -> Action {
        self.outcomes[self.current] = if skip || status == Status::NotTested { Outcome::Skipped } else { Outcome::Checked };
        if self.current + 1 < STEPS.len() {
            self.current += 1;
            Action::Goto(self.step())
        } else {
            Action::Finish
        }
    }

    fn back(&mut self) -> Action {
        if self.current > 0 {
            self.current -= 1;
            self.outcomes[self.current] = Outcome::Pending;
        }
        Action::Goto(self.step())
    }

    /// Steps the user skipped (or left untested), for the report.
    pub fn skipped(&self) -> Vec<Step> {
        STEPS.iter().zip(self.outcomes).filter(|(_, o)| *o == Outcome::Skipped).map(|(s, _)| *s).collect()
    }

    /// The guide bar. `status` is the current step's live result; `on_tab` says whether
    /// the user is looking at this step's tab; `available` is false when the hardware
    /// isn't there (no controller), so the step will be skipped.
    pub fn ui(&mut self, ui: &mut egui::Ui, status: Status, on_tab: bool, available: bool) -> Action {
        let mut action = Action::None;
        let step = self.step();
        let n = STEPS.len();
        theme::card().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            // Step dots with names.
            let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
            let p = ui.painter();
            let gap = row.width() / n as f32;
            let y = row.top() + 10.0;
            p.line_segment(
                [Pos2::new(row.left() + gap / 2.0, y), Pos2::new(row.right() - gap / 2.0, y)],
                Stroke::new(2.0, alpha(theme::LIGHT, 0.15)),
            );
            for (i, s) in STEPS.iter().enumerate() {
                let c = Pos2::new(row.left() + gap * (i as f32 + 0.5), y);
                let cur = i == self.current;
                match (cur, self.outcomes[i]) {
                    (true, _) => {
                        theme::glow(p, c, 18.0, alpha(Color32::WHITE, 0.35));
                        p.circle_filled(c, 8.0, Color32::WHITE);
                    }
                    (_, Outcome::Checked) => {
                        p.circle_filled(c, 7.0, theme::SILVER);
                        p.text(c, Align2::CENTER_CENTER, "✔", theme::bold(9.0), theme::INK);
                    }
                    (_, Outcome::Skipped) => {
                        p.circle_stroke(c, 6.0, Stroke::new(1.5, alpha(theme::SILVER, 0.5)));
                    }
                    (_, Outcome::Pending) => {
                        p.circle_filled(c, 6.0, Color32::from_rgb(40, 40, 44));
                        p.circle_stroke(c, 6.0, Stroke::new(1.0, alpha(theme::LIGHT, 0.3)));
                    }
                }
                let col = if cur { Color32::WHITE } else { theme::TEXT_DIM };
                p.text(c + Vec2::new(0.0, 19.0), Align2::CENTER_CENTER, t(s.name()), theme::bold(12.0), col);
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(tr!("Quick check · step {0} of {1}", self.current + 1, n))
                                .size(12.5)
                                .color(theme::TEXT_DIM),
                        );
                        badge(ui, status, available);
                    });
                    let how = if available { t(step.how()) } else { t("No controller is connected, so this step will be skipped. Plug one in to test it.") };
                    ui.label(egui::RichText::new(how).font(theme::bold(14.5)).color(Color32::WHITE));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = self.current + 1 == n;
                    let done = matches!(status, Status::Pass | Status::Issue);
                    let next = if last { t("Finish ✔") } else { t("Next ▶") };
                    let (c1, c2) = if done || !available { (theme::ACCENT, theme::SILVER) } else { (theme::LIGHT, theme::STEEL) };
                    if theme::fancy_button(ui, next, Vec2::new(120.0, 36.0), c1, c2, true).clicked() {
                        action = self.advance(status, !available);
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("Skip"), Vec2::new(80.0, 36.0), theme::SILVER, theme::STEEL, true).clicked() {
                        action = self.advance(status, true);
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("◀ Back"), Vec2::new(90.0, 36.0), theme::SILVER, theme::STEEL, self.current > 0).clicked() {
                        action = self.back();
                    }
                    if !on_tab {
                        ui.add_space(6.0);
                        if theme::fancy_button(ui, t("Show this step"), Vec2::new(130.0, 36.0), theme::LIGHT, theme::SILVER, true).clicked() {
                            action = Action::Goto(step);
                        }
                    }
                    ui.add_space(6.0);
                    if theme::fancy_button(ui, t("✕ Exit"), Vec2::new(80.0, 36.0), Color32::from_rgb(70, 70, 76), theme::STEEL, true)
                        .on_hover_text(t("Stop the quick check (your results are kept)"))
                        .clicked()
                    {
                        action = Action::Exit;
                    }
                });
            });
        });
        action
    }
}

/// The step's live result, as a small pill.
fn badge(ui: &mut egui::Ui, status: Status, available: bool) {
    let (text, fill, ink) = if !available {
        (t("Will be skipped"), alpha(theme::SILVER, 0.15), theme::TEXT_DIM)
    } else {
        match status {
            Status::Pass => (t("✔ Looks good"), Color32::WHITE, theme::INK),
            Status::Issue => (t("⚠ Issue found"), theme::GLINT, theme::INK),
            Status::InProgress => (t("… In progress"), alpha(theme::SILVER, 0.35), Color32::WHITE),
            Status::NotTested => (t("Not started"), alpha(theme::SILVER, 0.15), theme::TEXT_DIM),
        }
    };
    egui::Frame::new().fill(fill).corner_radius(10).inner_margin(egui::Margin::symmetric(8, 1)).show(ui, |ui| {
        ui.label(egui::RichText::new(text).font(theme::bold(12.0)).color(ink));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_record_checked_and_skipped() {
        let mut q = QuickCheck::new();
        assert_eq!(q.advance(Status::Pass, false), Action::Goto(Step::Keyboard));
        assert_eq!(q.advance(Status::Pass, false), Action::Goto(Step::Mouse));
        assert_eq!(q.advance(Status::NotTested, false), Action::Goto(Step::Display)); // untested → skipped
        assert_eq!(q.advance(Status::InProgress, true), Action::Goto(Step::Speakers)); // Skip pressed
        assert_eq!(q.outcomes[..4], [Outcome::Checked, Outcome::Checked, Outcome::Skipped, Outcome::Skipped]);
        assert_eq!(q.skipped(), vec![Step::Mouse, Step::Display]);
    }

    #[test]
    fn back_reopens_the_previous_step_and_the_last_step_finishes() {
        let mut q = QuickCheck::new();
        q.advance(Status::Pass, false);
        assert_eq!(q.back(), Action::Goto(Step::System));
        assert_eq!(q.outcomes[0], Outcome::Pending);
        q.current = STEPS.len() - 1;
        assert_eq!(q.advance(Status::Pass, false), Action::Finish);
    }
}
