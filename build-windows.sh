#!/usr/bin/env bash
# Cross-compile hwtest.exe for Windows (x86_64) from Linux.
# Output: dist/hwtest.exe
#
# Needs:
#   - rustup with the stable toolchain and the x86_64-pc-windows-gnu target
#   - the MinGW-w64 cross compiler (x86_64-w64-mingw32-gcc and -windres)
# The script checks for each one and explains how to install anything missing.
set -euo pipefail
cd "$(dirname "$0")"

TARGET=x86_64-pc-windows-gnu

fail() {
    printf '\n\033[1;31mError:\033[0m %s\n\n' "$1" >&2
    shift
    printf '%s\n' "$@" >&2
    exit 1
}

# 1. rustup: use the one on PATH, or a home-folder install that isn't on PATH.
RUSTUP=""
for candidate in "$(command -v rustup || true)" "$HOME/.cargo/bin/rustup" "$HOME/.rustup/cargo/bin/rustup"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
        RUSTUP="$candidate"
        break
    fi
done
[ -n "$RUSTUP" ] || fail "rustup was not found." \
    "Building the Windows .exe needs rustup, the official Rust installer, because" \
    "it can add the Windows target (a distro 'rust' package usually can't)." \
    "" \
    "Install it with:" \
    "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh" \
    "or from your package manager, e.g.  sudo pacman -S rustup  (Arch)." \
    "More options: https://rustup.rs" \
    "" \
    "Then run:  rustup target add $TARGET"

# 2. The Windows target for the stable toolchain.
if ! "$RUSTUP" target list --installed --toolchain stable 2>/dev/null | grep -qx "$TARGET"; then
    fail "the Rust target $TARGET is not installed." \
        "Add it with:" \
        "    $RUSTUP target add $TARGET --toolchain stable"
fi

# 3. The MinGW-w64 linker and resource compiler (for the icon).
for tool in x86_64-w64-mingw32-gcc x86_64-w64-mingw32-windres; do
    command -v "$tool" >/dev/null || fail "$tool was not found (MinGW-w64)." \
        "Install the MinGW-w64 cross compiler:" \
        "    Arch:          sudo pacman -S mingw-w64-gcc" \
        "    Debian/Ubuntu: sudo apt install gcc-mingw-w64-x86-64" \
        "    Fedora:        sudo dnf install mingw64-gcc"
done

# Use rustup's toolchain explicitly, even if a distro rustc/cargo is first on PATH.
CARGO="$("$RUSTUP" which cargo --toolchain stable)"
export RUSTC="$("$RUSTUP" which rustc --toolchain stable)"
export PATH="$(dirname "$RUSTC"):$PATH"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CARGO_PROFILE_RELEASE_STRIP=true

"$CARGO" build --release --locked --target "$TARGET"
mkdir -p dist
cp "target/$TARGET/release/hwtest.exe" dist/hwtest.exe
echo "Built: $(realpath dist/hwtest.exe)"
