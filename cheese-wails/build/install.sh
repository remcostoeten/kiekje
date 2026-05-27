#!/usr/bin/env bash
set -euo pipefail

APP_ID="kiekje"
PREFIX="${PREFIX:-$HOME/.local}"
BIN_DIR="$PREFIX/bin"
ICON_DIR="$PREFIX/share/icons/hicolor/256x256/apps"
DESKTOP_DIR="$PREFIX/share/applications"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR" "$HOME/.config/autostart"

install -m 755 "$SCRIPT_DIR/$APP_ID" "$BIN_DIR/$APP_ID"
install -m 755 "$SCRIPT_DIR/kiekje-tray" "$BIN_DIR/kiekje-tray"
install -m 644 "$SCRIPT_DIR/$APP_ID.png" "$ICON_DIR/$APP_ID.png"

cat > "$DESKTOP_DIR/$APP_ID.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Kiekje
Comment=Screenshot capture and annotation tool
Exec=$BIN_DIR/$APP_ID
Icon=$APP_ID
Terminal=false
Categories=Utility;Graphics;
StartupNotify=false
EOF

cp "$DESKTOP_DIR/$APP_ID.desktop" "$HOME/.config/autostart/$APP_ID.desktop"

"$BIN_DIR/$APP_ID" --sync-hyprland

echo "Installed Kiekje to $BIN_DIR/$APP_ID"
echo "Hyprland snippet synced via --sync-hyprland"
