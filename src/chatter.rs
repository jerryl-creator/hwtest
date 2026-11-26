//! Key chatter detection: a worn or dirty switch can register one press as two.
//!
//! People can't press the same key twice within a few hundredths of a second, so a second
//! press of the same key that soon after the first is reported as chatter.

use eframe::egui::Key;
use std::collections::HashMap;

/// Two presses of the same key closer together than this count as chatter.
/// Key events are timed per frame (about 17 ms apart at 60 fps), so this catches
/// presses in the same frame or the next one. Even very fast double taps by a person
/// are around 80 ms or more apart.
pub const CHATTER_SECS: f64 = 0.035;

#[derive(Default)]
pub struct ChatterDetector {
    last_press: HashMap<Key, f64>,
    /// How many times each key chattered.
    pub counts: HashMap<Key, u32>,
}

impl ChatterDetector {
    /// Record a (non-repeat) press at time `t` seconds. Returns true if it was chatter.
    pub fn press(&mut self, key: Key, t: f64) -> bool {
        let chatter = self
            .last_press
            .insert(key, t)
            .is_some_and(|prev| t - prev < CHATTER_SECS);
        if chatter {
            *self.counts.entry(key).or_default() += 1;
        }
        chatter
    }

    pub fn clear(&mut self) {
        self.last_press.clear();
        self.counts.clear();
    }

    pub fn count(&self, key: Key) -> u32 {
        self.counts.get(&key).copied().unwrap_or(0)
    }

    /// Chattering keys, worst first.
    pub fn worst(&self) -> Vec<(Key, u32)> {
        let mut v: Vec<_> = self.counts.iter().map(|(k, n)| (*k, *n)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name().cmp(b.0.name())));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_frame_double_press_is_chatter() {
        let mut d = ChatterDetector::default();
        assert!(!d.press(Key::E, 1.000));
        assert!(d.press(Key::E, 1.000));
        assert_eq!(d.count(Key::E), 1);
    }

    #[test]
    fn next_frame_double_press_is_chatter() {
        let mut d = ChatterDetector::default();
        d.press(Key::E, 1.000);
        assert!(d.press(Key::E, 1.017));
    }

    #[test]
    fn fast_human_double_tap_is_not_chatter() {
        let mut d = ChatterDetector::default();
        d.press(Key::E, 1.000);
        assert!(!d.press(Key::E, 1.080));
        assert!(d.counts.is_empty());
    }

    #[test]
    fn different_keys_are_independent() {
        let mut d = ChatterDetector::default();
        d.press(Key::E, 1.000);
        assert!(!d.press(Key::R, 1.001));
    }

    #[test]
    fn worst_sorts_by_count_and_clear_resets() {
        let mut d = ChatterDetector::default();
        for t in [0.0, 0.01, 0.02] {
            d.press(Key::A, t);
        }
        d.press(Key::B, 0.0);
        d.press(Key::B, 0.01);
        assert_eq!(d.worst(), vec![(Key::A, 2), (Key::B, 1)]);
        d.clear();
        assert!(d.worst().is_empty());
        assert!(!d.press(Key::A, 0.02));
    }
}
