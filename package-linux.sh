#!/usr/bin/env bash
# Build Nyanspect for Linux (x86_64) and package it for release:
#   dist/nyanspect-linux-x86_64.tar.gz
#     nyanspect/nyanspect           the app
#     nyanspect/install.sh          per-user installer (app menu + icons, no sudo)
#     nyanspect/nyanspect.desktop   app-menu entry template
#     nyanspect/icons/...
# Needs rustup (or cargo) and the ALSA development headers (see README).
set -euo pipefail
cd "$(dirname "$0")"

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
tar -C dist -czf dist/nyanspect-linux-x86_64.tar.gz nyanspect
echo "Built: $(realpath dist/nyanspect-linux-x86_64.tar.gz)"
