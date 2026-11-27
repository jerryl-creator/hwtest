//! Microphone test: live level meter, waveform, and "record 3 s and play back".
//!
//! The microphone is only opened when the user presses Start, and closed again when they
//! press Stop or leave the Audio tab. Recordings stay in memory and are never saved.

use crate::speakers::SpeakerTest;
use crate::i18n::t;
use crate::theme::{self, alpha, Mascot};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

const RECORD_SECS: f32 = 3.0;
const HISTORY: usize = 200;

#[derive(Default)]
struct Shared {
    /// Recent block peaks (0..1), newest last.
    history: VecDeque<f32>,
    /// RMS of the latest block (0..1).
    rms: f32,
    /// Mono samples being recorded, if a recording is in progress.
    recording: Option<Vec<f32>>,
    /// A finished recording waiting to be played back.
    finished: Option<Vec<f32>>,
}

pub struct MicTest {
    shared: Arc<Mutex<Shared>>,
    stream: Option<cpal::Stream>,
    rate: u32,
    device_name: String,
    error: Option<String>,
    started_at: f64,
    peak_hold: f32,
    /// Loudest level seen, in dBFS (for the report).
    pub max_db: Option<f32>,
    /// After playback: did the user hear their recording clearly?
    pub playback_ok: Option<bool>,
    played_back: bool,
    /// Has any non-zero sample arrived since starting? Pure digital silence means muted/blocked.
    signal_seen: bool,
}

impl Default for MicTest {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(Shared::default())),
            stream: None,
            rate: 0,
            device_name: String::new(),
            error: None,
            started_at: 0.0,
            peak_hold: 0.0,
            max_db: None,
            playback_ok: None,
            played_back: false,
            signal_seen: false,
        }
    }
}

fn to_db(x: f32) -> f32 {
    20.0 * x.max(1e-6).log10()
}

impl MicTest {
    pub fn is_on(&self) -> bool {
        self.stream.is_some()
    }

    /// Forget levels and the playback answer.
    pub fn clear_results(&mut self) {
        self.stop();
        self.max_db = None;
        self.playback_ok = None;
        self.signal_seen = false;
        self.error = None;
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        let started = !self.device_name.is_empty() || self.error.is_some();
        let silent = started && !self.signal_seen;
        let status = if self.error.is_some() || self.playback_ok == Some(false) || silent {
            Status::Issue
        } else if !started {
            Status::NotTested
        } else if self.playback_ok == Some(true) {
            Status::Pass
        } else {
            Status::InProgress
        };
        let mut lines = Vec::new();
        if let Some(e) = &self.error {
            lines.push(tr!("Couldn't open the microphone: {0}", e));
        }
        if !self.device_name.is_empty() {
            lines.push(tr!("Input: {0}", self.device_name));
        }
        if silent {
            lines.push(t("No signal at all: microphone muted or blocked?").into());
        } else if let Some(db) = self.max_db {
            lines.push(tr!("Loudest level: {0} dB", format!("{db:.0}")));
        }
        lines.push(match self.playback_ok {
            Some(true) => t("Recording played back clearly").into(),
            Some(false) => t("Recording sounded bad (quiet, noisy or crackly)").into(),
            None => t("Record & play back: not tried").into(),
        });
        Section { title: "Microphone", jp: "マイク", status, lines }
    }

    pub fn stop(&mut self) {
        self.stream = None;
        if let Ok(mut s) = self.shared.lock() {
            s.recording = None;
        }
    }

    fn start(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No microphone found")?;
        self.device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Default microphone".into());
        let config = device.default_input_config().map_err(|e| e.to_string())?;
        let stream_config: cpal::StreamConfig = config.clone().into();
        let channels = stream_config.channels.max(1) as usize;
        self.rate = stream_config.sample_rate;
        let target = (self.rate as f32 * RECORD_SECS) as usize;
        let shared = self.shared.clone();
        let err_fn = |e| eprintln!("microphone error: {e}");

        macro_rules! build {
            ($t:ty) => {
                device.build_input_stream(
                    stream_config.clone(),
                    move |data: &[$t], _| {
                        let Ok(mut s) = shared.try_lock() else { return };
                        let mut peak = 0.0f32;
                        let mut sum = 0.0f32;
                        let mut frames = 0usize;
                        for frame in data.chunks(channels) {
                            let v: f32 = frame.iter().map(|x| <f32 as cpal::FromSample<$t>>::from_sample_(*x)).sum::<f32>() / channels as f32;
                            peak = peak.max(v.abs());
                            sum += v * v;
                            frames += 1;
                            let done = match s.recording.as_mut() {
                                Some(rec) => {
                                    rec.push(v);
                                    rec.len() >= target
                                }
                                None => false,
                            };
                            if done {
                                s.finished = s.recording.take();
                            }
                        }
                        if frames > 0 {
                            s.rms = (sum / frames as f32).sqrt();
                            s.history.push_back(peak);
                            while s.history.len() > HISTORY {
                                s.history.pop_front();
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            };
        }
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build!(f32),
            cpal::SampleFormat::I16 => build!(i16),
            cpal::SampleFormat::U16 => build!(u16),
            cpal::SampleFormat::I32 => build!(i32),
            f => return Err(format!("Unsupported sample format {f:?}")),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.stream = Some(stream);
        Ok(())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool, speakers: &SpeakerTest) {
        let now = ui.input(|i| i.time);
        let (history, rms, recording_progress, finished) = {
            let mut s = self.shared.lock().unwrap();
            let prog = s.recording.as_ref().map(|r| r.len() as f32 / (self.rate as f32 * RECORD_SECS));
            (s.history.iter().copied().collect::<Vec<_>>(), s.rms, prog, s.finished.take())
        };
        if let Some(clip) = finished {
            speakers.play_clip(&clip, self.rate);
            self.played_back = true;
            self.playback_ok = None;
        }
        let playing_back = speakers.clip_playing();
        let latest = history.last().copied().unwrap_or(0.0);
        if self.is_on() {
            self.signal_seen |= history.iter().any(|v| *v > 0.0);
            self.peak_hold = (self.peak_hold - 0.4 * ui.input(|i| i.stable_dt)).max(latest);
            if latest > 0.0 {
                let db = to_db(latest);
                self.max_db = Some(self.max_db.map_or(db, |m| m.max(db)));
            }
        }

        if self.is_on() {
            if recording_progress.is_some() {
                mascot.say(t("Recording… say something cute~ (・∀・)"));
            } else if playing_back {
                mascot.say(t("Here's what your mic heard! ♪"));
            } else if now - self.started_at > 2.0 && !self.signal_seen {
                mascot.say(t("Total silence… is your mic muted? (；ω；)"));
            } else if now - self.started_at > 3.0 && self.max_db.is_none_or(|d| d < -70.0) {
                mascot.say(t("I can't hear anything… check your mic (；ω；)"));
            } else {
                mascot.say(t("Talk, clap or snap your fingers~ ♪"));
            }
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Microphone", "マイク", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (label, c1, c2) = if self.is_on() {
                        (t("■  Stop mic"), Color32::from_rgb(120, 120, 126), theme::STEEL)
                    } else {
                        (t("🎤  Start mic"), theme::ACCENT, theme::SILVER)
                    };
                    if theme::fancy_button(ui, label, Vec2::new(150.0, 36.0), c1, c2, true).clicked() {
                        if self.is_on() {
                            self.stop();
                        } else {
                            self.error = self.start().err();
                            self.started_at = now;
                            self.signal_seen = false;
                        }
                    }
                    ui.add_space(8.0);
                    let can_record = self.is_on() && recording_progress.is_none() && !playing_back;
                    if theme::fancy_button(ui, t("●  Record 3 s & play back"), Vec2::new(230.0, 36.0), theme::LIGHT, theme::SILVER, can_record).clicked() {
                        if let Ok(mut s) = self.shared.lock() {
                            s.recording = Some(Vec::with_capacity((self.rate as f32 * RECORD_SECS) as usize));
                        }
                    }
                    if self.is_on() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(&self.device_name).size(12.5).color(theme::TEXT_DIM));
                    }
                });
            });

            if let Some(e) = &self.error {
                ui.colored_label(theme::ACCENT, tr!("Couldn't open the microphone: {0}", e));
            }

            // Level meter (-60 dB … 0 dB) with peak hold.
            let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(bar, 11.0, Color32::from_rgba_unmultiplied(10, 10, 12, 210));
            let level = |x: f32| ((to_db(x) + 60.0) / 60.0).clamp(0.0, 1.0);
            let shown = ui.ctx().animate_value_with_time(ui.scope_id().with("mic-lvl"), if self.is_on() { level(rms * 1.41) } else { 0.0 }, 0.08);
            if shown > 0.01 {
                let fill = Rect::from_min_size(bar.min, Vec2::new((bar.width() * shown).max(22.0), bar.height()));
                theme::hgradient(p, fill, 11.0, theme::STEEL, Color32::WHITE);
            }
            if self.is_on() && self.peak_hold > 0.0 {
                let x = bar.left() + bar.width() * level(self.peak_hold);
                p.line_segment([Pos2::new(x, bar.top() + 3.0), Pos2::new(x, bar.bottom() - 3.0)], Stroke::new(2.5, Color32::WHITE));
            }
            let label = if !self.is_on() {
                t("Microphone off").to_string()
            } else if let Some(prog) = recording_progress {
                tr!("● Recording… {0} s", format!("{:.1}", prog * RECORD_SECS))
            } else if playing_back {
                t("▶ Playing back").to_string()
            } else if now - self.started_at > 2.0 && !self.signal_seen {
                t("No signal: is the microphone muted or blocked?").to_string()
            } else {
                format!("{:.0} dB", to_db(latest))
            };
            p.text(bar.center(), Align2::CENTER_CENTER, label, theme::bold(12.5), if shown > 0.5 { theme::INK } else { Color32::WHITE });

            // Waveform of recent peaks.
            ui.add_space(6.0);
            let (wave, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 70.0), Sense::hover());
            let p = ui.painter();
            p.rect_filled(wave, 12.0, Color32::from_rgba_unmultiplied(6, 6, 8, 210));
            if self.is_on() && history.len() > 1 {
                let step = wave.width() / HISTORY as f32;
                let x0 = wave.right() - history.len() as f32 * step;
                for (i, v) in history.iter().enumerate() {
                    let h = (level(*v) * wave.height() * 0.45).max(1.0);
                    let x = x0 + i as f32 * step;
                    p.line_segment([Pos2::new(x, wave.center().y - h), Pos2::new(x, wave.center().y + h)], Stroke::new(step.max(1.0) * 0.7, alpha(Color32::WHITE, 0.35 + 0.65 * i as f32 / history.len() as f32)));
                }
            }

            ui.add_space(6.0);
            if self.played_back && !playing_back {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t("Did your recording sound clear?")).font(theme::bold(13.5)).color(Color32::WHITE));
                    let opts = [(Some(true), t("Yes"), t("It sounded clear")), (Some(false), t("No"), t("Quiet, noisy, crackly or silent"))];
                    theme::segmented(ui, "mic-ok", &opts, &mut self.playback_ok);
                });
            }
            ui.label(
                egui::RichText::new(t(
                    "If the meter doesn't move: check the app has microphone access (Windows: Settings → Privacy & security → Microphone) and that the right input device is selected in your sound settings. Recordings are never saved.",
                ))
                .size(12.0)
                .color(theme::TEXT_DIM),
            );
        });

        if self.is_on() {
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::to_db;

    #[test]
    fn db_scale() {
        assert!((to_db(1.0) - 0.0).abs() < 1e-4);
        assert!((to_db(0.1) + 20.0).abs() < 1e-3);
        assert!(to_db(0.0).is_finite());
    }
}
