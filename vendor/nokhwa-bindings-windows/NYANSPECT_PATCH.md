# Patched nokhwa-bindings-windows 0.4.6

This is a copy of [nokhwa-bindings-windows](https://github.com/l1npengtul/nokhwa) 0.4.6
(license: Apache-2.0, copyright l1npengtul and the Nokhwa contributors), used through
`[patch.crates-io]` in the top-level `Cargo.toml`. It is the Windows (Media Foundation) camera
back-end of `nokhwa`, which Nyanspect uses for its Camera test on Windows only; on Linux the app
talks to Video4Linux2 directly.

One change in `src/lib.rs`, marked `nyanspect patch`:

**Frame rates.** Media Foundation describes a frame rate as a fraction (numerator / denominator).
The original code kept a rate only when the denominator was exactly 1 and treated everything else
as "no rate", so modes such as 30000/1001 (29.97 fps) or 10000000/333333 (30 fps) were dropped; a
webcam offering only such modes looked like it had no usable formats, and opening it failed. It
also read the camera's current frame rate as the raw denominator. Both now use
`mf_ratio_to_fps`, which divides and rounds (with a unit test). The same helper is used when
listing formats and when choosing one, so the two always agree.

To update: copy the new version from crates.io, re-apply the change, and keep the version number
in `Cargo.toml` equal to the one `nokhwa` asks for (otherwise Cargo ignores the patch).
