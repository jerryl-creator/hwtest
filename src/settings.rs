//! Settings remembered between launches (saved by eframe in the user's data folder).

use crate::layouts::{Size, Style};
use crate::speakers::Wave;
use serde::{Deserialize, Serialize};

pub const KEY: &str = "nyanspect-settings";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub kb_size: Size,
    pub kb_style: Style,
    pub sfx: bool,
    pub volume: f32,
    pub freq: f32,
    pub wave: Wave,
    pub sweep_secs: f32,
    pub auto_cycle: bool,
    pub cycle_secs: f32,
    /// Interface language the user picked (None = follow the system).
    pub lang: Option<crate::i18n::Lang>,
    /// No falling petals or page animations.
    pub reduce_motion: bool,
    /// Show Neko-chan in the top bar.
    pub show_mascot: bool,
    /// Text size (egui zoom factor).
    pub ui_scale: f32,
    /// Dark, light ("paper") or follow the system.
    pub theme: crate::theme::ThemeChoice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            kb_size: Size::Full,
            kb_style: Style::Ansi,
            sfx: true,
            volume: 0.3,
            freq: 440.0,
            wave: Wave::Sine,
            sweep_secs: 10.0,
            auto_cycle: false,
            cycle_secs: 3.0,
            lang: None,
            reduce_motion: false,
            show_mascot: true,
            ui_scale: 1.0,
            theme: crate::theme::ThemeChoice::Dark,
        }
    }
}
