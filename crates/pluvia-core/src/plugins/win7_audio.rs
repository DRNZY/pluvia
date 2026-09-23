use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Win7AudioQueryType {
    Volume,
    Device,
}

/// Linux native Win7Audio plugin emulating master audio volume, mute state, and device query.
#[derive(Debug, Clone)]
pub struct Win7AudioPlugin {
    volume: f64,
    muted: bool,
    device_name: String,
    is_mock: bool,
    query_type: Win7AudioQueryType,
    current_value: MeasureValue,
}

impl Win7AudioPlugin {
    /// Create a new Win7Audio plugin instance.
    pub fn new() -> Self {
        Self {
            volume: 50.0,
            muted: false,
            device_name: "Default Audio Sink".to_string(),
            is_mock: false,
            query_type: Win7AudioQueryType::Volume,
            current_value: MeasureValue::Number(50.0),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut plugin = Self::new();
        let target_str = config
            .get("type")
            .or_else(|| config.get("target"))
            .unwrap_or("volume")
            .to_ascii_lowercase();

        if target_str == "device" || target_str == "devicename" || target_str == "name" {
            plugin.query_type = Win7AudioQueryType::Device;
        }

        plugin
    }

    /// Configure mock volume, mute state, and device name for tests.
    pub fn with_mock(mut self, volume: f64, muted: bool, device_name: &str) -> Self {
        self.volume = volume.clamp(0.0, 100.0);
        self.muted = muted;
        self.device_name = device_name.to_string();
        self.is_mock = true;
        self
    }

    /// Retrieve current volume level (0.0 to 100.0).
    pub fn get_volume(&self) -> f64 {
        self.volume
    }

    /// Retrieve mute state.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Retrieve current audio device name.
    pub fn get_device_name(&self) -> &str {
        &self.device_name
    }

    /// Set volume percentage (clamped to 0.0..100.0).
    pub fn set_volume(&mut self, vol: f64) {
        self.volume = vol.clamp(0.0, 100.0);
        if !self.is_mock {
            self.sync_system_volume();
        }
    }

    /// Change volume by delta percentage (clamped to 0.0..100.0).
    pub fn change_volume(&mut self, delta: f64) {
        self.volume = (self.volume + delta).clamp(0.0, 100.0);
        if !self.is_mock {
            self.sync_system_volume();
        }
    }

    /// Set mute status.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        if !self.is_mock {
            self.sync_system_mute();
        }
    }

    /// Toggle mute status.
    pub fn toggle_mute(&mut self) {
        self.set_muted(!self.muted);
    }

    /// Set device name.
    pub fn set_device_name(&mut self, name: &str) {
        self.device_name = name.to_string();
    }

    /// Process Rainmeter bang commands (e.g. `"SetVolume 75"`, `"ChangeVolume +5"`, `"ToggleMute"`).
    pub fn command(&mut self, cmd: &str) {
        let trimmed = cmd.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("setvolume") {
            let rest = trimmed["setvolume".len()..].trim();
            if let Ok(vol) = rest.parse::<f64>() {
                self.set_volume(vol);
            }
        } else if lower.starts_with("changevolume") {
            let rest = trimmed["changevolume".len()..].trim();
            if let Ok(delta) = rest.parse::<f64>() {
                self.change_volume(delta);
            }
        } else if lower == "togglemute" {
            self.toggle_mute();
        } else if lower.starts_with("setmute") {
            let rest = trimmed["setmute".len()..].trim();
            match rest {
                "1" | "true" => self.set_muted(true),
                "0" | "false" => self.set_muted(false),
                _ => {}
            }
        }
    }

    fn query_system_state(&mut self) {
        // Try wpctl first (PipeWire standard)
        if let Ok(output) = Command::new("wpctl")
            .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // Output format: "Volume: 0.50 [MUTED]" or "Volume: 0.75"
                let is_muted = text.contains("[MUTED]");
                self.muted = is_muted;
                if let Some(vol_str) = text.split_whitespace().nth(1) {
                    if let Ok(frac) = vol_str.parse::<f64>() {
                        self.volume = (frac * 100.0).clamp(0.0, 100.0);
                    }
                }
                return;
            }
        }

        // Fallback to pactl (PulseAudio)
        if let Ok(output) = Command::new("pactl")
            .args(["get-sink-volume", "@DEFAULT_SINK@"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // Look for percentages like "50%"
                for part in text.split_whitespace() {
                    if part.ends_with('%') {
                        if let Ok(pct) = part[..part.len() - 1].parse::<f64>() {
                            self.volume = pct.clamp(0.0, 100.0);
                            break;
                        }
                    }
                }
            }
        }
    }

    fn sync_system_volume(&self) {
        let frac = self.volume / 100.0;
        let _ = Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{:.2}", frac)])
            .status();
    }

    fn sync_system_mute(&self) {
        let flag = if self.muted { "1" } else { "0" };
        let _ = Command::new("wpctl")
            .args(["set-mute", "@DEFAULT_AUDIO_SINK@", flag])
            .status();
    }
}

impl Default for Win7AudioPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for Win7AudioPlugin {
    fn update(&mut self) -> MeasureValue {
        if !self.is_mock {
            self.query_system_state();
        }

        let val = match self.query_type {
            Win7AudioQueryType::Device => MeasureValue::String(self.device_name.clone()),
            Win7AudioQueryType::Volume => {
                if self.muted {
                    MeasureValue::Number(-1.0)
                } else {
                    MeasureValue::Number(self.volume)
                }
            }
        };

        self.current_value = val;
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn command(&mut self, cmd: &str) {
        self.command(cmd);
    }
}
