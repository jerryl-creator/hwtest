#!/usr/bin/env bash
# Cross-compile hwtest.exe for Windows (x86_64) from Linux.
# Needs: rustup toolchain in ~/.rustup with the x86_64-pc-windows-gnu target, and mingw-w64-gcc.
set -euo pipefail
cd "$(dirname "$0")"
T="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin"
export RUSTUP_HOME="$HOME/.rustup" RUSTC="$T/rustc" PATH="$T:$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CARGO_PROFILE_RELEASE_STRIP=true
"$T/cargo" build --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/hwtest.exe ../hwtest.exe
echo "Built: $(realpath ../hwtest.exe)"
