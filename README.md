# Pluvia

A native Rainmeter desktop widget runtime and management tool for Linux. Basically Rainmeter for linux lol

## Disclaimer

This software is currently in early alpha (`v0.1.0-alpha`). It's experimental, dont get ya hopes up.

## Features

- Native parser for Rainmeter `.ini` configuration files.
- Cairo and Pango vector rendering engine.
- Libadwaita management GUI (`pluvia-studio`) with live variable tuning, offset adjustments, and skin library browser.
- Background daemon (`pluvia-daemon`) driving desktop surfaces via X11 and Wayland layer-shell.
- Command-line controller (`pluvia-cli`) for headless and scriptable skin control.
- Support for extracting and importing `.rmskin` packages.

## Prerequisites

On Arch Linux / Manjaro:
```bash
sudo pacman -S base-devel rust cairo pango gtk4 libadwaita xorg-server-devel
```

On Ubuntu / Debian (23.04+):
```bash
sudo apt update
sudo apt install build-essential cargo libgtk-4-dev libadwaita-1-dev libcairo2-dev libpango1.0-dev libx11-dev libxext-dev libxfixes-dev
```

## Installation

Clone the repository and build in release mode:

```bash
git clone https://github.com/DRNZY/pluvia.git
cd pluvia
cargo build --release
```

Copy the binaries to your local bin directory:

```bash
mkdir -p ~/.local/bin
cp target/release/pluvia-daemon ~/.local/bin/
cp target/release/pluvia-studio ~/.local/bin/
cp target/release/pluvia-cli ~/.local/bin/
chmod +x ~/.local/bin/pluvia-*
```

Install the desktop launcher for the studio:

```bash
mkdir -p ~/.local/share/applications
cp crates/pluvia-studio/resources/org.pluvia.Studio.desktop ~/.local/share/applications/
update-desktop-database ~/.local/share/applications/ 2>/dev/null || true
```

## Quick Start

### Graphical Interface
Launch Pluvia Studio from your application menu or run:
```bash
pluvia-studio
```
If the daemon is not running, click "Start Engine" in the studio header to launch the background process.

### Command Line
Start the background daemon manually:
```bash
pluvia-daemon &
```

Load a skin:
```bash
pluvia-cli load ~/.config/pluvia/skins/Monterey/Widgets/Clock/Medium.ini
```

List active skins:
```bash
pluvia-cli list
```

Unload a skin:
```bash
pluvia-cli unload Clock/Medium
```

Import a `.rmskin` package:
```bash
pluvia-cli import /path/to/skin.rmskin
```

## Skin Directory

Place installed Rainmeter skins inside:
```
~/.config/pluvia/skins/
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
