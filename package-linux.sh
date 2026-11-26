#!/usr/bin/env bash
# Build Nyanspect for Linux and package it for release, named for this machine's CPU:
#   dist/nyanspect-linux-x86_64.tar.gz   (64-bit Intel / AMD)
#   dist/nyanspect-linux-arm64.tar.gz    (64-bit ARM)
#     nyanspect/nyanspect           the app
#     nyanspect/install.sh          per-user installer (app menu + icons, no sudo)
#     nyanspect/nyanspect.desktop   app-menu entry template
#     nyanspect/icons/...
# Needs rustup (or cargo) and the ALSA development headers (see README).
set -euo pipefail
cd "$(dirname "$0")"

case "$(uname -m)" in
    x86_64) ARCH=x86_64 ;;
    aarch64 | arm64) ARCH=arm64 ;;
    *) echo "Unsupported CPU type: $(uname -m)" >&2; exit 1 ;;
esac
PACKAGE="nyanspect-linux-$ARCH.tar.gz"

CARGO_PROFILE_RELEASE_STRIP=true cargo build --release --locked

STAGE=dist/nyanspect
rm -rf "$STAGE"
mkdir -p "$STAGE/icons"
cp target/release/nyanspect "$STAGE/"
cp packaging/linux/install.sh packaging/linux/nyanspect.desktop "$STAGE/"
for s in 16 32 48 64 128 256; do cp "assets/icon-$s.png" "$STAGE/icons/nyanspect-$s.png"; done
cp assets/icon.svg "$STAGE/icons/nyanspect.svg"
cp LICENSE "$STAGE/"
chmod +x "$STAGE/nyanspect" "$STAGE/install.sh"
tar -C dist -czf "dist/$PACKAGE" nyanspect
echo "Built: $(realpath "dist/$PACKAGE")"
