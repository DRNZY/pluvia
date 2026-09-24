<div align="center">

# Pluvia

<p>
  <strong>Rainmeter skins on Linux.</strong><br/>
  <em>Rust, Cairo vector graphics, Wayland and X11.</em>
</p>

<p>
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Wayland-000000?style=for-the-badge&logo=wayland&logoColor=white" alt="Wayland" />
  <img src="https://img.shields.io/badge/X11-000000?style=for-the-badge&logo=xorg&logoColor=white" alt="X11" />
  <img src="https://img.shields.io/badge/License-MIT-181818?style=for-the-badge" alt="MIT License" />
</p>

<p align="center">
  <img src="docs/pluvia-launch.gif" width="100%" alt="Pluvia launch demo" />
</p>

</div>

---

Pluvia is a desktop engine that parses standard Rainmeter `.ini` skins and renders them natively on Linux desktops using Wayland layer-shell and X11.

## Quick install

Run the install script to set up the daemon, CLI, and Pluvia Studio:

```bash
curl -sSL https://raw.githubusercontent.com/DRNZY/pluvia/main/scripts/install.sh | bash
```

---

## Features

- Parse standard `.ini` skins, math formulas, dynamic variables, and custom measures.
- Native click-through transparency on Wayland layer-shell and X11.
- Real-time audio visualization using PipeWire and PulseAudio.
- Pluvia Studio GUI (GTK4 / libadwaita) to manage skins, adjust positions, scale, and edit variables live.
- Isolated worker threads for telemetry measures so desktop rendering never stutters.
- Import and unpack `.rmskin` packages directly.

---

## Usage

### GUI

Launch Pluvia Studio from your application menu or run:

```bash
pluvia-studio
```

### CLI

Start the daemon:

```bash
pluvia-daemon &
```

Load a skin:

```bash
pluvia-cli load ~/.config/pluvia/skins/Mond/Clock/Clock.ini
```

List active skins:

```bash
pluvia-cli list
```

Unload a skin:

```bash
pluvia-cli unload Mond/Clock
```

Import a `.rmskin` archive:

```bash
pluvia-cli import /path/to/skin.rmskin
```

---

## Building from source

### Dependencies

- Arch / CachyOS / Manjaro:
  ```bash
  sudo pacman -S base-devel rust cairo pango gtk4 libadwaita xorg-server-devel
  ```
- Ubuntu / Debian:
  ```bash
  sudo apt update && sudo apt install build-essential cargo libgtk-4-dev libadwaita-1-dev libcairo2-dev libpango1.0-dev libx11-dev libxext-dev libxfixes-dev
  ```

### Build and install

```bash
git clone https://github.com/DRNZY/pluvia.git
cd pluvia
bash scripts/install-local.sh
```

---

## License

MIT. Built and maintained by [@DRNZY](https://github.com/DRNZY).
