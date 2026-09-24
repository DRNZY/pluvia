<div align="center">

# ⚡ Pluvia

<p>
  <strong>High-Performance Rainmeter Skin Engine for Linux</strong><br/>
  <em>Pure Rust · 100% Vector Cairo Graphics · Wayland LayerShell & X11 Click-Through</em>
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

## ⚡ Quick Install

Install Pluvia daemon, CLI, and Pluvia Studio with a single command:

```bash
curl -sSL https://raw.githubusercontent.com/DRNZY/pluvia/main/scripts/install.sh | bash
```

---

## ✨ Features

- **Vector Graphics Engine:** Sub-millisecond Cairo and Pango vector compositing with automatic frame damage tracking.
- **True Desktop Click-Through:** 1-bit alpha shape masks with diff detection for X11 and Wayland `wlr-layer-shell`, keeping transparent widget areas 100% interactive.
- **Native Audio Spectrum Visualizers:** Zero-latency audio capture via PipeWire and PulseAudio monitor sinks (`@DEFAULT_MONITOR@`). Parent-child FFT snapshot registry eliminates redundant calculations across multi-bar meters.
- **Pluvia Studio:** Native GTK4 / Libadwaita management control center with live variable tuning, window opacity sliders, and skin browsers.
- **Deadlock-Free Thread Isolation:** Dedicated OS worker threads for MPRIS2 media controllers and D-Bus IPC with non-blocking timeout channels.
- **RMSKIN Package Manager:** Safe package importer and exporter with Zip-Slip path traversal protection.

---

## 🚀 Quick Start

### Graphical Interface
Launch **Pluvia Studio** from your application menu or run:
```bash
pluvia-studio
```

### Command Line Interface
Start the background engine:
```bash
pluvia-daemon &
```

Load any Rainmeter skin:
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

## 🛠️ Building from Source

### Prerequisites
- **Arch Linux / CachyOS / Manjaro:**
  ```bash
  sudo pacman -S base-devel rust cairo pango gtk4 libadwaita xorg-server-devel
  ```
- **Ubuntu / Debian (23.04+):**
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

This project is licensed under the [MIT License](LICENSE). Maintained with passion by **[@DRNZY](https://github.com/DRNZY)**.

