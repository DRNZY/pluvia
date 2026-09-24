<div align="center">

# ⚡ Pluvia

<p>
  <strong>Rainmeter skins, running natively on Linux.</strong><br/>
  <em>Written in 100% Rust · Cairo Vector Graphics · Wayland & X11</em>
</p>

<p>
  <img src="https://img.shields.io/badge/RUST-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/WAYLAND-000000?style=for-the-badge&logo=wayland&logoColor=white" alt="Wayland" />
  <img src="https://img.shields.io/badge/X11-000000?style=for-the-badge&logo=xorg&logoColor=white" alt="X11" />
  <img src="https://img.shields.io/badge/LICENSE-MIT-181818?style=for-the-badge" alt="MIT License" />
</p>

<p align="center">
  <img src="docs/pluvia-launch.gif" width="100%" alt="Pluvia Launch Demo" />
</p>

</div>

---

No Wine, no heavy webview wrappers, and no background memory leaks. Pluvia is a native desktop engine that parses standard Rainmeter `.ini` skins and renders them smoothly on Linux desktops.

## ⚡ Quick Install

Run this in your terminal to install the daemon, CLI, and Pluvia Studio:

```bash
curl -sSL https://raw.githubusercontent.com/DRNZY/pluvia/main/scripts/install.sh | bash
```

---

## What it does

- **Runs real Rainmeter skins:** Parses standard `.ini` files, math formulas, dynamic variables, and custom measures.
- **Click-through transparency:** Transparent parts of widgets let clicks pass straight through to your desktop or background windows (supports both Wayland LayerShell and X11).
- **Desktop Audio Visualizers:** Hooks into PipeWire/PulseAudio to visualize your music playback in real-time without listening to your microphone.
- **Pluvia Studio:** A clean GTK4/Adwaita control app to manage your skins, adjust coordinates, change scale, and edit variables live.
- **Fast and lightweight:** Written in pure Rust with isolated background worker threads so your desktop stays responsive.
- **`.rmskin` installer:** Extract and install `.rmskin` packages directly.

---

## 🚀 How to use it

### Pluvia Studio (GUI)
Open **Pluvia Studio** from your app launcher or run:
```bash
pluvia-studio
```

### Command Line (CLI)
Start the background daemon:
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

Install a `.rmskin` package:
```bash
pluvia-cli import /path/to/skin.rmskin
```

---

## 🛠️ Building from Source

If you prefer building it yourself:

### Dependencies
- **Arch / CachyOS / Manjaro:**
  ```bash
  sudo pacman -S base-devel rust cairo pango gtk4 libadwaita xorg-server-devel
  ```
- **Ubuntu / Debian:**
  ```bash
  sudo apt update && sudo apt install build-essential cargo libgtk-4-dev libadwaita-1-dev libcairo2-dev libpango1.0-dev libx11-dev libxext-dev libxfixes-dev
  ```

### Build & Install
```bash
git clone https://github.com/DRNZY/pluvia.git
cd pluvia
bash scripts/install-local.sh
```

---

## 📜 License

MIT License. Built and maintained by **[@DRNZY](https://github.com/DRNZY)**.


