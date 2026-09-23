pub mod calc_measure;
pub mod gpu;
pub mod mpris;
pub mod plugin_fallback;
pub mod power;
pub mod string_measure;
pub mod substitute;
pub mod system;
pub mod thermal;
pub mod time;

use crate::ini::MeasureConfig;
use crate::variables::VariableMap;
use std::collections::HashMap;

/// Value produced by a telemetry measure.
#[derive(Debug, Clone, PartialEq)]
pub enum MeasureValue {
    String(String),
    Number(f64),
}

impl MeasureValue {
    /// Return the string representation of the measure value.
    pub fn to_string_val(&self) -> String {
        match self {
            MeasureValue::String(s) => s.clone(),
            MeasureValue::Number(n) => {
                if n.fract() == 0.0 && !n.is_infinite() && !n.is_nan() && n.abs() < 1e16 {
                    format!("{:.0}", n)
                } else {
                    format!("{}", n)
                }
            }
        }
    }

    /// Return the numeric representation of the measure value.
    pub fn to_number_val(&self) -> f64 {
        match self {
            MeasureValue::Number(n) => *n,
            MeasureValue::String(s) => {
                let trimmed = s.trim();
                if let Ok(val) = trimmed.parse::<f64>() {
                    return val;
                }
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() == 3 {
                    if let (Ok(h), Ok(m), Ok(sec)) = (
                        parts[0].parse::<f64>(),
                        parts[1].parse::<f64>(),
                        parts[2].parse::<f64>(),
                    ) {
                        return h * 3600.0 + m * 60.0 + sec;
                    }
                } else if parts.len() == 2 {
                    if let (Ok(h), Ok(m)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                        return h * 3600.0 + m * 60.0;
                    }
                }
                0.0
            }
        }
    }

    /// Access inner string slice if value is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            MeasureValue::String(s) => Some(s.as_str()),
            MeasureValue::Number(_) => None,
        }
    }

    /// Access inner number if value is numeric.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MeasureValue::Number(n) => Some(*n),
            MeasureValue::String(_) => None,
        }
    }
}

impl std::fmt::Display for MeasureValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_val())
    }
}

impl From<String> for MeasureValue {
    fn from(s: String) -> Self {
        MeasureValue::String(s)
    }
}

impl From<&str> for MeasureValue {
    fn from(s: &str) -> Self {
        MeasureValue::String(s.to_string())
    }
}

impl From<f64> for MeasureValue {
    fn from(n: f64) -> Self {
        MeasureValue::Number(n)
    }
}

impl From<u64> for MeasureValue {
    fn from(n: u64) -> Self {
        MeasureValue::Number(n as f64)
    }
}

impl From<i64> for MeasureValue {
    fn from(n: i64) -> Self {
        MeasureValue::Number(n as f64)
    }
}

/// Common trait for all Rainmeter telemetry and data collection measures.
pub trait Measure: Send + Sync {
    /// Update the measure state and return the updated value.
    fn update(&mut self) -> MeasureValue;

    /// Get the current/cached value without updating.
    fn get_value(&self) -> MeasureValue;

    /// Update with skin context (variables and other measures).
    fn update_with_context(
        &mut self,
        _vars: &VariableMap,
        _measures: &HashMap<String, MeasureValue>,
    ) -> MeasureValue {
        self.update()
    }

    /// Dispatch an interactive command to this measure (e.g. from !CommandMeasure).
    fn command(&mut self, _cmd: &str) {}

    /// Feed audio samples for spectrum analysis (e.g. AudioLevel measures).
    fn feed_audio(&mut self, _samples: &[f32]) {}
}

/// Factory function to instantiate a measure from a parsed `MeasureConfig`.
pub fn create_measure(config: &MeasureConfig) -> Option<Box<dyn Measure>> {
    let m_type = config.measure_type.to_ascii_lowercase();
    match m_type.as_str() {
        "time" => Some(Box::new(time::TimeMeasure::from_config(config))),
        "string" => Some(Box::new(string_measure::StringMeasure::from_config(config))),
        "calc" => Some(Box::new(calc_measure::CalcMeasure::from_config(config))),
        "cpu" => Some(Box::new(system::CpuMeasure::from_config(config))),
        "physicalmemory" | "memory" | "swapmemory" => {
            Some(Box::new(system::MemoryMeasure::from_config(config)))
        }
        "freediskspace" | "disk" => Some(Box::new(system::DiskMeasure::from_config(config))),
        "uptime" => Some(Box::new(system::UptimeMeasure::from_config(config))),
        "netin" | "netout" | "nettotal" | "net" => {
            Some(Box::new(system::NetMeasure::from_config(config)))
        }
        "thermal" | "temperature" | "coretemp" | "speedfan" => {
            Some(Box::new(thermal::ThermalMeasure::from_config(config)))
        }
        "power" | "battery" | "powerstate" | "powerstatus" | "powerplugin" => {
            Some(Box::new(power::PowerMeasure::from_config(config)))
        }
        "gpu" | "gpumonitor" | "msiafterburner" | "hwinfo" => {
            Some(Box::new(gpu::GpuMeasure::from_config(config)))
        }
        "nowplaying" => Some(Box::new(mpris::NowPlayingMeasure::from_config(config))),
        "actiontimer" => Some(Box::new(crate::plugins::action_timer::ActionTimerPlugin::from_config(config))),
        "audiolevel" => Some(Box::new(crate::plugins::audio_level::AudioLevelPlugin::from_config(config))),
        "win7audio" => Some(Box::new(crate::plugins::win7_audio::Win7AudioPlugin::from_config(config))),
        "process" => Some(Box::new(crate::plugins::process::ProcessPlugin::from_config(config))),
        "webparser" => Some(Box::new(crate::plugins::web_parser::WebParserPlugin::from_config(config))),
        "plugin" => {
            let plugin_raw = config.plugin.as_deref().unwrap_or("").to_ascii_lowercase();
            let plugin = plugin_raw.strip_suffix(".dll").unwrap_or(&plugin_raw);
            match plugin {
                "nowplaying" => Some(Box::new(mpris::NowPlayingMeasure::from_config(config))),
                "actiontimer" => Some(Box::new(crate::plugins::action_timer::ActionTimerPlugin::from_config(config))),
                "audiolevel" => Some(Box::new(crate::plugins::audio_level::AudioLevelPlugin::from_config(config))),
                "win7audio" => Some(Box::new(crate::plugins::win7_audio::Win7AudioPlugin::from_config(config))),
                "process" => Some(Box::new(crate::plugins::process::ProcessPlugin::from_config(config))),
                "webparser" => Some(Box::new(crate::plugins::web_parser::WebParserPlugin::from_config(config))),
                "coretemp" | "speedfan" => Some(Box::new(thermal::ThermalMeasure::from_config(config))),
                "powerplugin" | "batteryplugin" | "battery" => Some(Box::new(power::PowerMeasure::from_config(config))),
                "msiafterburner" | "hwinfo" | "gpumonitor" => Some(Box::new(gpu::GpuMeasure::from_config(config))),
                "usagemonitor" | "perfmon" => {
                    let cat = config.get("category").unwrap_or("").to_ascii_lowercase();
                    if cat.contains("gpu") {
                        Some(Box::new(gpu::GpuMeasure::from_config(config)))
                    } else if cat.contains("memory") || cat.contains("ram") {
                        Some(Box::new(system::MemoryMeasure::from_config(config)))
                    } else {
                        Some(Box::new(system::CpuMeasure::from_config(config)))
                    }
                }
                _ => Some(Box::new(plugin_fallback::FallbackPluginMeasure::from_config(config))),
            }
        }
        _ => None,
    }
}
