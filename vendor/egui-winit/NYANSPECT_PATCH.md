# Patched egui-winit 0.37.0

This is a copy of [egui-winit](https://github.com/emilk/egui/tree/main/crates/egui-winit) 0.37.0
(license: MIT OR Apache-2.0), used through `[patch.crates-io]` in the top-level `Cargo.toml`.

Two small changes in `src/lib.rs`, both marked `nyanspect patch`:

1. **Numpad keys.** egui has no separate numpad keys (Numpad1 is reported as Num1, and NumpadEnter as Enter).
   The *physical* key of numpad presses is reported as F20–F35 instead, which real keyboards don't have
   (see `numpad_as_f_key`). The logical key is unchanged, so typing still works normally.
2. **Clipboard shortcuts.** Ctrl+C/X/V are normally turned into Copy/Cut/Paste events with no key event,
   so those keys wouldn't light up in the keyboard test. That interception is switched off; the app never
   uses the clipboard.
