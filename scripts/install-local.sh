#!/usr/bin/env bash
set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_BIN="$HOME/.local/bin"
INSTALL_APPS="$HOME/.local/share/applications"
INSTALL_ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
SYSTEMD_USER_DIR="$HOME/.config/systemd/user"

echo "[Pluvia Installer] Compiling release binaries (pluvia-cli, pluvia-daemon, pluvia-studio)..."
cd "$PROJECT_DIR"
cargo build --release

mkdir -p "$INSTALL_BIN" "$INSTALL_APPS" "$INSTALL_ICONS" "$SYSTEMD_USER_DIR"

echo "[Pluvia Installer] Installing binaries to $INSTALL_BIN..."
cp "$PROJECT_DIR/target/release/pluvia-cli" "$INSTALL_BIN/pluvia"
cp "$PROJECT_DIR/target/release/pluvia-cli" "$INSTALL_BIN/pluvia-cli"
cp "$PROJECT_DIR/target/release/pluvia-daemon" "$INSTALL_BIN/pluvia-daemon"
cp "$PROJECT_DIR/target/release/pluvia-studio" "$INSTALL_BIN/pluvia-studio"

chmod +x "$INSTALL_BIN/pluvia" "$INSTALL_BIN/pluvia-cli" "$INSTALL_BIN/pluvia-daemon" "$INSTALL_BIN/pluvia-studio"

# Create Desktop Entry for Pluvia Studio
echo "[Pluvia Installer] Creating desktop entry at $INSTALL_APPS/pluvia-studio.desktop..."
cat << 'DESKTOP_EOF' > "$INSTALL_APPS/pluvia-studio.desktop"
[Desktop Entry]
Version=1.0
Type=Application
Name=Pluvia Studio
GenericName=Desktop Widget Manager
Comment=Native Rainmeter skin engine and widget manager for Linux
Exec=pluvia-studio
Icon=preferences-desktop-wallpaper
Terminal=false
Categories=Utility;DesktopSettings;GTK;
StartupWMClass=pluvia-studio
Keywords=Rainmeter;Widgets;Skins;Desktop;Clock;Monitor;
DESKTOP_EOF

# Create Systemd User Service
echo "[Pluvia Installer] Creating systemd service at $SYSTEMD_USER_DIR/pluvia.service..."
cat << 'SYSTEMD_EOF' > "$SYSTEMD_USER_DIR/pluvia.service"
[Unit]
Description=Pluvia Rainmeter Desktop Engine for Linux
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/pluvia-daemon
Restart=on-failure
RestartSec=3s
Slice=app-graphical.slice

[Install]
WantedBy=graphical-session.target
SYSTEMD_EOF

# Reload user systemd daemon
systemctl --user daemon-reload 2>/dev/null || true
update-desktop-database "$INSTALL_APPS" 2>/dev/null || true

echo "[Pluvia Installer] Successfully installed Pluvia ecosystem to $INSTALL_BIN."
