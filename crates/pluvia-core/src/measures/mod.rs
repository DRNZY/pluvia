pub mod mpris;
pub mod system;
pub mod time;

use crate::ini::MeasureConfig;

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
            MeasureValue::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
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
}

/// Factory function to instantiate a measure from a parsed `MeasureConfig`.
pub fn create_measure(config: &MeasureConfig) -> Option<Box<dyn Measure>> {
    let m_type = config.measure_type.to_ascii_lowercase();
    match m_type.as_str() {
        "time" => Some(Box::new(time::TimeMeasure::from_config(config))),
        "cpu" => Some(Box::new(system::CpuMeasure::from_config(config))),
        "physicalmemory" | "memory" | "swapmemory" => {
            Some(Box::new(system::MemoryMeasure::from_config(config)))
        }
        "freediskspace" | "disk" => Some(Box::new(system::DiskMeasure::from_config(config))),
        "uptime" => Some(Box::new(system::UptimeMeasure::from_config(config))),
        "netin" | "netout" | "nettotal" | "net" => {
            Some(Box::new(system::NetMeasure::from_config(config)))
        }
        "nowplaying" => Some(Box::new(mpris::NowPlayingMeasure::from_config(config))),
        "plugin" => {
            let plugin = config.plugin.as_deref().unwrap_or("").to_ascii_lowercase();
            if plugin == "nowplaying" || plugin == "nowplaying.dll" {
                Some(Box::new(mpris::NowPlayingMeasure::from_config(config)))
            } else {
                None
            }
        }
        _ => None,
    }
}
