<div align="center">

# Pluvia

<p>
  <strong>Rainmeter skins on Linux.</strong><br/>
  <em>Rust, Cairo vector graphics, Wayland and X11.</em>
</p>

<p>
  <img src="https://img.shields.io/badge/status-EARLY%20ALPHA-ff4d00?style=for-the-badge" alt="Early alpha" />
  <img src="https://img.shields.io/badge/version-0.1.0--alpha.1-blue?style=for-the-badge" alt="v0.1.0-alpha.1" />
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Wayland-000000?style=for-the-badge&logo=wayland&logoColor=white" alt="Wayland" />
  <img src="https://img.shields.io/badge/X11-000000?style=for-the-badge&logo=xorg&logoColor=white" alt="X11" />
  <img src="https://img.shields.io/badge/License-GPL--3.0-181818?style=for-the-badge" alt="GPL-3.0 License" />
</p>

> **Early alpha.** This is release `0.1.0-alpha.1`. Layout and rendering work, but plenty
> of skins are still unsupported and things will change between releases. Back up your
> skin folder before pointing it at your real `.ini` files, and please open an issue when
> something breaks.

<p align="center">
  <img src="docs/pluvia-launch.gif" width="100%" alt="Pluvia launch demo" />
</p>

</div>

---

Pluvia renders standard Rainmeter `.ini` skins natively on Linux, on both Wayland
(layer-shell) and X11.

## Install

Clone the repo and run the install script. It builds the daemon, CLI and Studio, then
puts the binaries in `~/.local/bin`:

```bash
git clone https://github.com/DRNZY/pluvia.git
cd pluvia
bash scripts/install-local.sh
```

## Features

- Parses standard `.ini` skins, including math formulas, dynamic variables and custom
  measures
- Click-through transparency on Wayland layer-shell and X11
- Live audio visualization through PipeWire and PulseAudio
- Pluvia Studio (GTK4 / libadwaita) for moving, scaling and editing skins
- Telemetry measures run on their own threads, so rendering stays smooth
- Imports `.rmskin` packages

## Usage

### Studio

Launch Pluvia Studio from your application menu, or run:

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

List the loaded skins:

```bash
pluvia-cli list
```

Unload one:

```bash
pluvia-cli unload Mond/Clock
```

Import a `.rmskin` archive:

```bash
pluvia-cli import /path/to/skin.rmskin
```

## Building from source

Install the dependencies first.

On Arch, CachyOS or Manjaro:

```bash
sudo pacman -S base-devel rust cairo pango gtk4 libadwaita xorg-server-devel
```

On Ubuntu or Debian:

```bash
sudo apt update && sudo apt install build-essential cargo libgtk-4-dev libadwaita-1-dev libcairo2-dev libpango1.0-dev libx11-dev libxext-dev libxfixes-dev
```

Then clone and install, which is the same command as the [install](#install) section above.

## License

GNU General Public License v3.0 (GPL-3.0). Built and maintained by [@DRNZY](https://github.com/DRNZY).
