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
        }
    }
}
