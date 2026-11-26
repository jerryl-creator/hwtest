//! Little synthesized sound effects for the manga pop-ups (no audio files needed).
//!
//! Each sound is one or more short "voices": a tone gliding from one pitch to another,
//! optionally mixed with filtered noise, shaped by a quick attack and exponential decay.

use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SfxSound {
    /// Bubbly pop for ordinary keys. `pitch` ≈ 0.8..1.25 so repeated presses vary.
    Pop { pitch: f32 },
    /// Space: deep boom (ドーン!!)
    Boom,
    /// Enter: two-note chime (決定!)
    Chime,
    /// Backspace / Delete: falling zap (消えろ!)
    Zap,
    /// Esc: double falling chirp (逃げろ!)
    Flee,
    /// Shift / Ctrl / Alt / Tab: soft whoosh (シュッ!)
    Whoosh,
    /// Arrow keys: rising zip (ビュン!)
    Zip,
    /// F1–F12: power-up sweep with sparkles (必殺技!)
    PowerUp,
    /// Win: little major chord (神!)
    Chord,
    /// Caps Lock: buzzy shout (叫べ!)
    Shout,
}

#[derive(Clone, Copy, PartialEq)]
enum Wave {
    Sine,
    Triangle,
    Square,
}

#[derive(Clone)]
pub struct Voice {
    /// Seconds to wait before this voice starts (for multi-note sounds).
    delay: f32,
    t: f32,
    dur: f32,
    f0: f32,
    f1: f32,
    wave: Wave,
    tone: f32,
    noise: f32,
    /// One-pole low-pass amount for the noise (0..1, smaller = darker).
    lp: f32,
    gain: f32,
    phase: f32,
    lp_state: f32,
    rng: u32,
}

impl Voice {
    #[allow(clippy::too_many_arguments)]
    fn new(delay: f32, dur: f32, f0: f32, f1: f32, wave: Wave, tone: f32, noise: f32, lp: f32, gain: f32) -> Self {
        Self { delay, t: 0.0, dur, f0, f1, wave, tone, noise, lp, gain, phase: 0.0, lp_state: 0.0, rng: 0x1357_9BDF }
    }

    pub fn finished(&self) -> bool {
        self.t >= self.delay + self.dur
    }

    /// Advance one sample and return its value.
    pub fn next(&mut self, dt: f32) -> f32 {
        self.t += dt;
        let t = self.t - self.delay;
        if t < 0.0 || t >= self.dur {
            return 0.0;
        }
        let x = t / self.dur;
        // exponential pitch glide f0 → f1
        let freq = self.f0 * (self.f1 / self.f0).powf(x);
        self.phase = (self.phase + freq * dt).fract();
        let p = self.phase;
        let osc = match self.wave {
            Wave::Sine => (p * TAU).sin(),
            Wave::Triangle => 1.0 - 4.0 * (p - 0.5).abs(),
            Wave::Square => {
                if p < 0.5 { 0.6 } else { -0.6 }
            }
        };
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let white = (self.rng as f32 / u32::MAX as f32) * 2.0 - 1.0;
        self.lp_state += (white - self.lp_state) * self.lp;
        // 3 ms attack, exponential decay, 5 ms fade-out at the end (no clicks)
        let attack = (t / 0.003).min(1.0);
        let decay = (-4.5 * x).exp();
        let release = ((self.dur - t) / 0.005).min(1.0);
        (osc * self.tone + self.lp_state * self.noise) * attack * decay * release * self.gain
    }
}

/// The voices that make up one sound effect.
pub fn voices_for(sound: SfxSound) -> Vec<Voice> {
    use Wave::*;
    let v = Voice::new;
    match sound {
        SfxSound::Pop { pitch } => vec![
            v(0.0, 0.09, 1100.0 * pitch, 380.0 * pitch, Sine, 1.0, 0.25, 0.6, 0.45),
        ],
        SfxSound::Boom => vec![
            v(0.0, 0.65, 150.0, 40.0, Sine, 1.0, 0.0, 0.0, 0.9),
            v(0.0, 0.35, 90.0, 60.0, Sine, 0.0, 1.2, 0.06, 0.55),
        ],
        SfxSound::Chime => vec![
            v(0.0, 0.30, 880.0, 880.0, Triangle, 1.0, 0.0, 0.0, 0.35),
            v(0.09, 0.40, 1320.0, 1320.0, Triangle, 1.0, 0.0, 0.0, 0.35),
        ],
        SfxSound::Zap => vec![
            v(0.0, 0.20, 900.0, 140.0, Square, 0.6, 0.4, 0.35, 0.35),
        ],
        SfxSound::Flee => vec![
            v(0.0, 0.10, 1600.0, 600.0, Sine, 1.0, 0.0, 0.0, 0.4),
            v(0.10, 0.12, 1300.0, 450.0, Sine, 1.0, 0.0, 0.0, 0.4),
        ],
        SfxSound::Whoosh => vec![
            v(0.0, 0.16, 300.0, 300.0, Sine, 0.0, 1.4, 0.18, 0.45),
        ],
        SfxSound::Zip => vec![
            v(0.0, 0.12, 380.0, 1900.0, Sine, 1.0, 0.1, 0.5, 0.4),
        ],
        SfxSound::PowerUp => vec![
            v(0.0, 0.28, 200.0, 900.0, Square, 0.7, 0.15, 0.3, 0.3),
            v(0.20, 0.18, 1760.0, 1760.0, Sine, 1.0, 0.0, 0.0, 0.3),
            v(0.27, 0.22, 2640.0, 2640.0, Sine, 1.0, 0.0, 0.0, 0.25),
        ],
        SfxSound::Chord => vec![
            v(0.0, 0.5, 523.0, 523.0, Triangle, 1.0, 0.0, 0.0, 0.25),
            v(0.06, 0.5, 659.0, 659.0, Triangle, 1.0, 0.0, 0.0, 0.25),
            v(0.12, 0.55, 784.0, 784.0, Triangle, 1.0, 0.0, 0.0, 0.25),
        ],
        SfxSound::Shout => vec![
            v(0.0, 0.25, 330.0, 250.0, Square, 0.8, 0.2, 0.4, 0.3),
        ],
    }
}

/// Mix and advance all voices by one sample, dropping finished ones.
/// Called from the audio callback: `retain` never allocates.
pub fn mix(voices: &mut Vec<Voice>, dt: f32) -> f32 {
    if voices.is_empty() {
        return 0.0;
    }
    let sum: f32 = voices.iter_mut().map(|v| v.next(dt)).sum();
    voices.retain(|v| !v.finished());
    // Master level: leaves headroom when several sounds overlap.
    sum * 0.8
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [SfxSound; 10] = [
        SfxSound::Pop { pitch: 1.0 },
        SfxSound::Boom,
        SfxSound::Chime,
        SfxSound::Zap,
        SfxSound::Flee,
        SfxSound::Whoosh,
        SfxSound::Zip,
        SfxSound::PowerUp,
        SfxSound::Chord,
        SfxSound::Shout,
    ];

    /// Render a sound to samples until all its voices finish.
    fn render(sound: SfxSound, sr: f32) -> Vec<f32> {
        let mut voices = voices_for(sound);
        let mut out = Vec::new();
        while !voices.is_empty() && out.len() < sr as usize * 2 {
            out.push(mix(&mut voices, 1.0 / sr));
        }
        out
    }

    #[test]
    fn every_sound_is_audible_finite_and_short() {
        for sound in ALL {
            let s = render(sound, 48_000.0);
            let peak = s.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(s.iter().all(|x| x.is_finite()), "{sound:?} produced NaN/inf");
            assert!(peak > 0.1, "{sound:?} is too quiet (peak {peak})");
            assert!(peak <= 1.0, "{sound:?} clips (peak {peak})");
            let secs = s.len() as f32 / 48_000.0;
            assert!(secs < 0.8, "{sound:?} lasts too long ({secs} s)");
            // ends smoothly: the last 2 ms are near silence (no click)
            let tail = &s[s.len() - 96..];
            assert!(tail.iter().all(|x| x.abs() < 0.02), "{sound:?} ends with a click");
        }
    }

    #[test]
    fn mix_drops_finished_voices() {
        let mut voices = voices_for(SfxSound::Pop { pitch: 1.0 });
        for _ in 0..48_000 {
            mix(&mut voices, 1.0 / 48_000.0);
        }
        assert!(voices.is_empty());
    }
}
