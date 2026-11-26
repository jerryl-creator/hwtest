#!/usr/bin/env bash
# Installs Nyanspect for the current user (no sudo needed):
#   ~/.local/bin/nyanspect                    the app
#   ~/.local/share/applications/...           app-menu entry
#   ~/.local/share/icons/hicolor/...          icons
# Uninstall:  ./install.sh --uninstall
set -euo pipefail
cd "$(dirname "$0")"

BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor"
SIZES="16 32 48 64 128 256"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$BIN/nyanspect" "$APPS/nyanspect.desktop" "$ICONS/scalable/apps/nyanspect.svg"
    for s in $SIZES; do rm -f "$ICONS/${s}x${s}/apps/nyanspect.png"; done
    echo "Nyanspect uninstalled."
    exit 0
fi

install -Dm755 nyanspect "$BIN/nyanspect"
for s in $SIZES; do install -Dm644 "icons/nyanspect-$s.png" "$ICONS/${s}x${s}/apps/nyanspect.png"; done
install -Dm644 icons/nyanspect.svg "$ICONS/scalable/apps/nyanspect.svg"
mkdir -p "$APPS"
sed "s|@BIN@|$BIN/nyanspect|" nyanspect.desktop > "$APPS/nyanspect.desktop"

# Refresh menu and icon caches where those tools exist (harmless if they don't).
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$ICONS" 2>/dev/null || true
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true

echo "Nyanspect installed. Open it from your app menu, or run: $BIN/nyanspect"
case ":$PATH:" in
    *":$BIN:"*) ;;
    *) echo "Note: $BIN is not on your PATH, so typing 'nyanspect' won't work yet (the app menu will)." ;;
esac
