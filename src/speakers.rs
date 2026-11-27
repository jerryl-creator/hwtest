use crate::sfx_audio::{self, SfxSound, Voice};
use crate::i18n::{t, tc};
use crate::theme::{self, alpha, lerp_color, Mascot};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Wave {
    Sine,
    Square,
    Noise,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Off,
    /// Steady tone on the selected channels.
    Tone,
    /// Bounce left → right every second.
    Alternate,
    /// Logarithmic sweep 20 Hz → 20 kHz over `sweep_secs`.
    Sweep,
}

/// State shared between the UI thread and the audio callback.
struct Shared {
    mode: Mode,
    wave: Wave,
    freq: f32,
    volume: f32,
    /// Channels the user selected.
    mask: Vec<bool>,
    /// Channels actually sounding right now (differs from `mask` in Alternate mode).
    live: Vec<bool>,
    sweep_secs: f32,
    phase: f32,
    elapsed: f32,
    rng: u32,
    current_freq: f32,
    /// Sound-effect voices currently playing (key-press SFX), mixed on top of the test tone.
    voices: Vec<Voice>,
    /// A recorded clip being played back (mic test), already at the output sample rate.
    clip: Vec<f32>,
    clip_pos: usize,
}

pub struct SpeakerTest {
    /// Per channel: did the user hear it? (None = not answered yet)
    pub heard: Vec<Option<bool>>,
    /// Per channel: has it been played at least once?
    played: Vec<bool>,
    shared: Arc<Mutex<Shared>>,
    _stream: Option<cpal::Stream>,
    channels: usize,
    sample_rate: u32,
    device_name: String,
    error: Option<String>,
}

fn channel_name(i: usize, n: usize) -> String {
    let names: &[&str] = match n {
        1 => &["Mono"],
        2 => &["Left", "Right"],
        4 => &["Front L", "Front R", "Rear L", "Rear R"],
        6 => &["Front L", "Front R", "Center", "Subwoofer", "Rear L", "Rear R"],
        8 => &["Front L", "Front R", "Center", "Subwoofer", "Rear L", "Rear R", "Side L", "Side R"],
        _ => &[],
    };
    names.get(i).map(|s| tc("speaker", s).to_string()).unwrap_or(tr!("Ch {0}", i + 1))
}

impl SpeakerTest {
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            mode: Mode::Off,
            wave: Wave::Sine,
            freq: 440.0,
            volume: 0.3,
            mask: vec![],
            live: vec![],
            sweep_secs: 10.0,
            phase: 0.0,
            elapsed: 0.0,
            rng: 0x1234_5678,
            current_freq: 440.0,
            voices: Vec::with_capacity(32),
            clip: Vec::new(),
            clip_pos: 0,
        }));
        let mut me = Self { heard: vec![], played: vec![], shared, _stream: None, channels: 0, sample_rate: 0, device_name: String::new(), error: None };
        if let Err(e) = me.open() {
            me.error = Some(e);
        }
        me
    }

    fn open(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("No audio output device found")?;
        self.device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "Default output".into());
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let stream_config: cpal::StreamConfig = config.clone().into();
        self.channels = stream_config.channels as usize;
        self.heard = vec![None; self.channels];
        self.played = vec![false; self.channels];
        self.sample_rate = stream_config.sample_rate;
        {
            let mut s = self.shared.lock().unwrap();
            s.mask = vec![true; self.channels];
            s.live = vec![false; self.channels];
        }

        let shared = self.shared.clone();
        let channels = self.channels;
        let sr = self.sample_rate as f32;
        let err_fn = |e| eprintln!("audio stream error: {e}");

        macro_rules! build {
            ($t:ty) => {
                device.build_output_stream(
                    stream_config.clone(),
                    move |data: &mut [$t], _| {
                        let Ok(mut s) = shared.try_lock() else {
                            data.fill(<$t as cpal::Sample>::EQUILIBRIUM);
                            return;
                        };
                        for frame in data.chunks_mut(channels) {
                            let v = next_sample(&mut s, sr);
                            let mut fx = sfx_audio::mix(&mut s.voices, 1.0 / sr);
                            if s.clip_pos < s.clip.len() {
                                fx += s.clip[s.clip_pos];
                                s.clip_pos += 1;
                            }
                            for (c, out) in frame.iter_mut().enumerate() {
                                let mut x = if s.live.get(c).copied().unwrap_or(false) { v } else { 0.0 };
                                // Sound effects play on the front left/right (or mono) speakers.
                                if fx != 0.0 && c < 2 {
                                    x = (x + fx).clamp(-1.0, 1.0);
                                }
                                *out = <$t as cpal::FromSample<f32>>::from_sample_(x);
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
        self._stream = Some(stream);
        Ok(())
    }

    pub fn load_settings(&mut self, s: &crate::settings::Settings) {
        if let Ok(mut sh) = self.shared.lock() {
            sh.volume = s.volume.clamp(0.0, 1.0);
            sh.freq = s.freq.clamp(20.0, 20_000.0);
            sh.wave = s.wave;
            sh.sweep_secs = s.sweep_secs.clamp(3.0, 30.0);
        }
    }

    pub fn store_settings(&self, s: &mut crate::settings::Settings) {
        if let Ok(sh) = self.shared.lock() {
            s.volume = sh.volume;
            s.freq = sh.freq;
            s.wave = sh.wave;
            s.sweep_secs = sh.sweep_secs;
        }
    }

    /// Play a key-press sound effect (no-op if there's no audio device).
    pub fn play_sfx(&self, sound: SfxSound) {
        if self._stream.is_none() {
            return;
        }
        if let Ok(mut s) = self.shared.lock() {
            // Keep it light: drop the oldest voices if many keys are mashed at once.
            let new = sfx_audio::voices_for(sound);
            let len = s.voices.len();
            let excess = (len + new.len()).saturating_sub(24).min(len);
            s.voices.drain(..excess);
            s.voices.extend(new);
        }
    }

    /// Play a mono clip (e.g. a mic recording) recorded at `rate` Hz on the front speakers.
    pub fn play_clip(&self, samples: &[f32], rate: u32) {
        if self._stream.is_none() || samples.is_empty() || rate == 0 {
            return;
        }
        let clip = resample(samples, rate, self.sample_rate);
        if let Ok(mut s) = self.shared.lock() {
            s.clip = clip;
            s.clip_pos = 0;
        }
    }

    pub fn clip_playing(&self) -> bool {
        self.shared.lock().map(|s| s.clip_pos < s.clip.len()).unwrap_or(false)
    }

    /// Forget which speakers were heard.
    pub fn clear_results(&mut self) {
        self.stop();
        self.heard = vec![None; self.channels];
        self.played = vec![false; self.channels];
    }

    pub fn report(&self) -> crate::report::Section {
        use crate::report::{Section, Status};
        if let Some(e) = &self.error {
            return Section { title: "Speakers", jp: "スピーカー", status: Status::Issue, lines: vec![tr!("No audio output: {0}", e)] };
        }
        let answered = self.heard.iter().filter(|h| h.is_some()).count();
        let bad = self.heard.iter().any(|h| *h == Some(false));
        let status = if bad {
            Status::Issue
        } else if answered == 0 {
            Status::NotTested
        } else if answered == self.heard.len() {
            Status::Pass
        } else {
            Status::InProgress
        };
        let mut lines = vec![tr!("Output: {0} ({1} channels)", self.device_name, self.channels)];
        for (c, h) in self.heard.iter().enumerate() {
            let what = t(match h {
                Some(true) => "heard ✔",
                Some(false) => "NOT heard (or distorted)",
                None => "not checked",
            });
            lines.push(format!("{}: {what}", channel_name(c, self.channels)));
        }
        Section { title: "Speakers", jp: "スピーカー", status, lines }
    }

    pub fn stop(&mut self) {
        if let Ok(mut s) = self.shared.lock() {
            s.mode = Mode::Off;
        }
    }

    fn start(&self, mode: Mode, mask: Vec<bool>) {
        let mut s = self.shared.lock().unwrap();
        s.mode = mode;
        s.mask = mask;
        s.elapsed = 0.0;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mascot: &mut Mascot, cjk: bool) {
        if let Some(e) = &self.error {
            theme::card().show(ui, |ui| {
                theme::card_title(ui, "Speaker test", "スピーカーテスト", cjk);
                ui.colored_label(theme::ACCENT, tr!("Audio unavailable: {0}", e));
            });
            mascot.say(t("Hmm… I can't find any speakers (；ω；)"));
            return;
        }

        let n = self.channels;
        let (mode, current, live, wave, volume) = {
            let s = self.shared.lock().unwrap();
            (s.mode, s.current_freq, s.live.clone(), s.wave, s.volume)
        };
        let playing = mode != Mode::Off;
        if playing {
            // The waveform and sweep readout animate while a tone plays.
            ui.ctx().request_repaint();
        }
        match mode {
            Mode::Off => mascot.say(t("Turn your volume down a bit first, ne~ (・ω・)")),
            Mode::Sweep => mascot.say(t("Listen for buzzing or rattling~ (｀・ω・´)")),
            Mode::Alternate => mascot.say(t("Left… right… left… right~ ♪")),
            Mode::Tone => mascot.say(t("Can you hear it? La la la~ ♫")),
        }
        if playing {
            mascot.cheer();
        }

        theme::card().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::card_title(ui, "Speaker test", "スピーカーテスト", cjk);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}  ·  {} ch  ·  {} Hz", self.device_name, n, self.sample_rate))
                            .size(12.5)
                            .color(theme::TEXT_DIM),
                    );
                });
            });

            // Speaker orbs
            let orb = 92.0;
            let spacing = 26.0;
            let row_w = n as f32 * orb + (n.saturating_sub(1)) as f32 * spacing;
            let (area, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), orb + 82.0), Sense::hover());
            let start_x = area.center().x - row_w / 2.0;
            let time = ui.input(|i| i.time as f32);
            for c in 0..n {
                let center = Pos2::new(start_x + orb / 2.0 + c as f32 * (orb + spacing), area.top() + orb / 2.0 + 10.0);
                let r = Rect::from_center_size(center, Vec2::splat(orb));
                let resp = ui.interact(r, ui.scope_id().with(("orb", c)), Sense::CLICK).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.clicked() {
                    let mut mask = vec![false; n];
                    mask[c] = true;
                    self.start(Mode::Tone, mask);
                    self.played[c] = true;
                }
                let on = playing && live.get(c).copied().unwrap_or(false);
                let a = ui.ctx().animate_bool_with_time(resp.id.with("on"), on, 0.2);
                let h = ui.ctx().animate_bool_with_time(resp.id.with("hov"), resp.hovered(), 0.15);
                draw_orb(ui.painter(), center, orb / 2.0, a, h, time, &channel_name(c, n));

                // Result badge and the "Heard it?" Yes/No buttons once this speaker has played.
                if let Some(ok) = self.heard[c] {
                    let b = center + Vec2::new(orb * 0.33, -orb * 0.33);
                    ui.painter().circle(b, 11.0, if ok { Color32::WHITE } else { theme::STEEL }, Stroke::new(1.5, theme::INK));
                    ui.painter().text(b, Align2::CENTER_CENTER, if ok { "✔" } else { "!" }, theme::bold(12.0), if ok { theme::INK } else { Color32::WHITE });
                }
                if self.played[c] {
                    let opts = [
                        (Some(true), t("Yes"), t("I heard this speaker")),
                        (Some(false), t("No"), t("No sound (or crackling) from this speaker")),
                    ];
                    let r = Rect::from_center_size(Pos2::new(center.x, area.top() + orb + 58.0), Vec2::new(orb + spacing, 34.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
                        let w = 86.0;
                        ui.add_space(((r.width() - w) / 2.0).max(0.0));
                        theme::segmented(ui, &format!("heard-{c}"), &opts, &mut self.heard[c]);
                    });
                }
            }

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let total = 4.0 * 186.0 + 3.0 * 10.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                let b = Vec2::new(186.0, 42.0);
                if theme::fancy_button(ui, t("♫  All channels"), b, theme::ACCENT, theme::SILVER, true).clicked() {
                    self.start(Mode::Tone, vec![true; n]);
                    self.played.iter_mut().for_each(|p| *p = true);
                }
                if theme::fancy_button(ui, t("⇄  Left / Right"), b, theme::SILVER, theme::STEEL, n >= 2).clicked() {
                    self.start(Mode::Alternate, vec![true; n]);
                }
                if theme::fancy_button(ui, t("~  Sweep 20 Hz – 20 kHz"), b, theme::LIGHT, theme::SILVER, true).clicked() {
                    self.start(Mode::Sweep, vec![true; n]);
                }
                if theme::fancy_button(ui, t("■  Stop"), b, Color32::from_rgb(120, 120, 126), theme::STEEL, playing).clicked() {
                    self.stop();
                }
            });
        });

        ui.add_space(14.0);
        ui.columns(2, |cols| {
            theme::card().show(&mut cols[0], |ui| {
                theme::card_title(ui, "Waveform", "波形", cjk);
                let freq = if mode == Mode::Sweep { current } else { self.shared.lock().unwrap().freq };
                visualizer(ui, playing, wave, freq, volume, ui.input(|i| i.time as f32));
                ui.add_space(6.0);
                let status = match mode {
                    Mode::Off => t("Stopped").to_string(),
                    Mode::Tone => {
                        let on: Vec<_> = live.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| channel_name(i, n)).collect();
                        tr!("Playing  ·  {0}", on.join(", "))
                    }
                    Mode::Alternate => {
                        let side = if live.first().copied().unwrap_or(false) { t("LEFT") } else { t("RIGHT") };
                        tr!("Alternating  ·  {0}", side)
                    }
                    Mode::Sweep => tr!("Sweeping  ·  {0} Hz", format!("{current:.0}")),
                };
                ui.label(egui::RichText::new(status).font(theme::bold(18.0)).color(if playing { theme::GLOW } else { theme::TEXT_DIM }));
            });

            theme::card().show(&mut cols[1], |ui| {
                theme::card_title(ui, "Settings", "設定", cjk);
                let mut s = self.shared.lock().unwrap();
                ui.horizontal(|ui| {
                    ui.label(t("Wave"));
                    ui.selectable_value(&mut s.wave, Wave::Sine, t("Sine"));
                    ui.selectable_value(&mut s.wave, Wave::Square, tc("wave", "Square"));
                    ui.selectable_value(&mut s.wave, Wave::Noise, t("White noise"));
                });
                ui.add(egui::Slider::new(&mut s.freq, 20.0..=20000.0).logarithmic(true).suffix(" Hz").text(t("Frequency")));
                ui.add(egui::Slider::new(&mut s.volume, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0}%", v * 100.0)).text(t("Volume")));
                ui.add(egui::Slider::new(&mut s.sweep_secs, 3.0..=30.0).suffix(" s").text(t("Sweep length")));
                ui.add_space(6.0);
                ui.label(egui::RichText::new(t("⚠ Start with the volume low — test tones can be loud.")).size(12.5).color(theme::GLINT));
            });
        });
    }
}

fn draw_orb(p: &egui::Painter, c: Pos2, r: f32, on: f32, hover: f32, t: f32, name: &str) {
    // sound-wave rings
    if on > 0.01 {
        for k in 0..3 {
            let ph = ((t * 1.2 + k as f32 / 3.0) % 1.0) as f32;
            p.circle_stroke(c, r * (0.8 + ph * 0.7), Stroke::new(3.0 * (1.0 - ph), alpha(theme::GLOW, on * (1.0 - ph))));
        }
        theme::glow(p, c, r * 1.6, alpha(theme::GLOW, 0.35 * on));
    }
    theme::glow(p, c, r * 1.2, alpha(theme::ACCENT, 0.15 + 0.2 * hover));
    let pulse = 1.0 + on * 0.05 * (t * 8.0).sin();
    let rr = r * 0.78 * pulse * (1.0 + hover * 0.04);
    let top = lerp_color(Color32::from_rgb(62, 62, 66), Color32::from_rgb(255, 255, 255), on);
    let bot = lerp_color(Color32::from_rgb(30, 30, 33), Color32::from_rgb(190, 190, 196), on);
    theme::vgradient(p, Rect::from_center_size(c, Vec2::splat(rr * 2.0)), rr, top, bot);
    p.circle_stroke(c, rr, Stroke::new(1.5, alpha(Color32::WHITE, 0.25 + 0.5 * hover.max(on))));
    // speaker cone
    p.circle_stroke(c, rr * 0.55, Stroke::new(2.0, lerp_color(alpha(Color32::WHITE, 0.45), theme::INK, on)));
    p.circle_filled(c, rr * 0.22 * (1.0 + on * 0.15 * (t * 20.0).sin()), lerp_color(alpha(Color32::WHITE, 0.6), theme::INK, on));
    p.text(c + Vec2::new(0.0, r + 16.0), Align2::CENTER_CENTER, name, theme::bold(13.5), lerp_color(theme::TEXT_DIM, Color32::WHITE, on.max(hover)));
}

fn visualizer(ui: &mut egui::Ui, playing: bool, wave: Wave, freq: f32, volume: f32, t: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 130.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 14.0, Color32::from_rgba_unmultiplied(6, 6, 8, 210));
    // grid
    for i in 1..4 {
        let y = rect.top() + rect.height() * i as f32 / 4.0;
        p.line_segment([Pos2::new(rect.left() + 8.0, y), Pos2::new(rect.right() - 8.0, y)], Stroke::new(1.0, alpha(theme::SILVER, 0.08)));
    }
    let amp_target = if playing { (0.25 + volume * 0.75).min(1.0) } else { 0.04 };
    let amp = ui.ctx().animate_value_with_time(ui.scope_id().with("viz-amp"), amp_target, 0.25);
    // Visual cycles across the view: log-scale so 20 Hz..20 kHz all look reasonable.
    let cycles = 1.5 + (freq.max(20.0) / 20.0).log10() * 3.0;
    let steps = 220;
    let mut rng = 0x2468_ACE1u32 ^ ((t * 30.0) as u32);
    let mut pts = Vec::with_capacity(steps);
    for i in 0..steps {
        let x = i as f32 / (steps - 1) as f32;
        let ph = (x * cycles - t * 1.5).rem_euclid(1.0);
        let y = match wave {
            Wave::Sine => (ph * TAU).sin(),
            Wave::Square => if ph < 0.5 { 0.8 } else { -0.8 },
            Wave::Noise => {
                rng ^= rng << 13;
                rng ^= rng >> 17;
                rng ^= rng << 5;
                (rng % 2000) as f32 / 1000.0 - 1.0
            }
        };
        // taper the ends
        let taper = (x * std::f32::consts::PI).sin().max(0.0).sqrt();
        pts.push(Pos2::new(rect.left() + 12.0 + x * (rect.width() - 24.0), rect.center().y - y * amp * taper * rect.height() * 0.38));
    }
    for (w, a) in [(9.0, 0.10), (5.0, 0.22)] {
        p.add(egui::Shape::line(pts.clone(), Stroke::new(w, alpha(theme::ACCENT, a))));
    }
    // gradient-coloured core line
    for (i, seg) in pts.windows(2).enumerate() {
        let c = lerp_color(theme::ACCENT, theme::GLOW, i as f32 / steps as f32);
        p.line_segment([seg[0], seg[1]], Stroke::new(2.2, c));
    }
}

/// Produce one sample and update `s.live` (which channels sound this frame).
fn next_sample(s: &mut Shared, sr: f32) -> f32 {
    let dt = 1.0 / sr;
    if s.mode == Mode::Off {
        s.live.iter_mut().for_each(|b| *b = false);
        return 0.0;
    }
    s.elapsed += dt;

    let freq = match s.mode {
        Mode::Sweep => {
            let t = (s.elapsed / s.sweep_secs).min(1.0);
            if t >= 1.0 {
                s.mode = Mode::Off;
            }
            20.0 * (1000.0f32).powf(t) // 20 Hz × 1000^t → 20 kHz
        }
        _ => s.freq,
    };
    s.current_freq = freq;

    let left = (s.elapsed as u32) % 2 == 0;
    for i in 0..s.live.len() {
        s.live[i] = match s.mode {
            // Even channels = left side, odd = right (holds for 2.0 / 5.1 / 7.1 pairs).
            Mode::Alternate => (i % 2 == 0) == left,
            _ => s.mask.get(i).copied().unwrap_or(false),
        };
    }

    s.phase = (s.phase + freq * dt) % 1.0;
    let raw = match s.wave {
        Wave::Sine => (s.phase * TAU).sin(),
        Wave::Square => if s.phase < 0.5 { 0.7 } else { -0.7 },
        Wave::Noise => {
            s.rng ^= s.rng << 13;
            s.rng ^= s.rng >> 17;
            s.rng ^= s.rng << 5;
            (s.rng as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    };
    raw * s.volume
}

/// Linear resampling from `from` Hz to `to` Hz.
fn resample(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if samples.is_empty() || from == 0 || to == 0 {
        return Vec::new();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (samples.len() as f64 / ratio) as usize;
    (0..out_len)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x as usize;
            let f = (x - j as f64) as f32;
            let a = samples[j.min(samples.len() - 1)];
            let b = samples[(j + 1).min(samples.len() - 1)];
            a + (b - a) * f
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::resample;

    #[test]
    fn resample_keeps_duration_and_shape() {
        // 1 second of a 100 Hz sine at 44.1 kHz → 48 kHz
        let src: Vec<f32> = (0..44_100).map(|i| (i as f32 / 44_100.0 * 100.0 * std::f32::consts::TAU).sin()).collect();
        let out = resample(&src, 44_100, 48_000);
        assert!((out.len() as i64 - 48_000).abs() <= 1);
        // Same instant (0.25 s) in both should have nearly the same value.
        assert!((out[12_000] - src[11_025]).abs() < 0.01);
        assert!(out.iter().all(|x| x.is_finite() && x.abs() <= 1.0 + 1e-6));
    }

    #[test]
    fn resample_same_rate_is_identity() {
        let src = vec![0.1, -0.2, 0.3, -0.4];
        assert_eq!(resample(&src, 48_000, 48_000), src);
    }
}
