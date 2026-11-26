<p align="center"><img src="assets/icon-128.png" width="96" alt="HW Test logo"></p>

<h1 align="center">HW Test</h1>
<p align="center">A small, anime-styled hardware tester for <b>keyboards</b>, <b>dead pixels</b> and <b>speakers</b>.<br>Written in Rust with <a href="https://github.com/emilk/egui">egui</a>. Runs on Windows 10/11 and Linux.</p>

![HW Test keyboard tab](docs/screenshot.png)

## Features

**⌨ Keyboard**
- Full-size Windows layout (104 keys), including the numpad, with left/right Shift, Ctrl, Alt and Win shown separately
- Keys turn silver once they work and glow white while held; a progress bar counts tested keys
- Manga sound-effect pop-ups on every key press (ドーン!! for Space, 決定! for Enter, …), which you can turn off with the SFX button

**★ Dead pixels**
- Fullscreen black, white, red, green, blue, cyan, magenta, yellow and gray
- Click, Space or → for the next colour, ← for the previous, Esc to exit; optional auto-cycle
- Optional stuck-pixel "fixer" (rapid RGB flashing, with a photosensitivity warning)

**♫ Speakers**
- One button per output channel (stereo, 4.0, 5.1, 7.1), plus all channels, left/right alternate, and a 20 Hz – 20 kHz sweep
- Sine, square or white noise, with adjustable frequency and volume, and a live waveform

## Download

Windows builds are attached to the [Releases](../../releases) page: a single `hwtest.exe`, no installer needed.
Windows may show a SmartScreen warning because the file isn't code-signed: choose **More info → Run anyway**.

## Building

Linux (needs a Rust toolchain and ALSA headers):

```bash
cargo build --release
./target/release/hwtest
```

Windows `.exe`, cross-compiled from Linux (needs `rustup` with the `x86_64-pc-windows-gnu` target and `mingw-w64-gcc`):

```bash
./build-windows.sh
```

## Graphics compatibility

HW Test prefers the integrated GPU (it doesn't need a discrete one, which saves battery on laptops) and uses DirectX 12 on Windows or Vulkan on Linux.
If that fails it restarts itself with OpenGL (`--opengl`), and if there's no usable GPU driver it falls back to a software renderer.
Startup errors are shown in a dialog and saved to `hwtest-error.log` in the temp folder.

## Notes

- `vendor/egui-winit` is a lightly patched copy of [egui-winit](https://github.com/emilk/egui) 0.37 (MIT OR Apache-2.0), used so the app can tell numpad keys apart from the main keys. See [vendor/egui-winit/HWTEST_PATCH.md](vendor/egui-winit/HWTEST_PATCH.md).
- Japanese labels use a font already on the system (Noto Sans CJK on Linux, Yu Gothic or Meiryo on Windows). Without one, they are simply hidden.

## License

HW Test is released under the [MIT License](LICENSE).
The patched copy of egui-winit in `vendor/egui-winit` keeps its original MIT OR Apache-2.0 license.
