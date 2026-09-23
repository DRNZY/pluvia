# Running Pluvia Daemon with Systemd

You can run `pluvia-daemon` as a user service under systemd.

## Service Unit

Create `~/.config/systemd/user/pluvia-daemon.service`:

```ini
[Unit]
Description=Pluvia Rainmeter Skin Daemon
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/pluvia-daemon
Restart=on-failure
RestartSec=3

[Install]
WantedBy=graphical-session.target
```

Enable and start:
```bash
systemctl --user enable --now pluvia-daemon.service
```
