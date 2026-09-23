use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Thermal metric type requested by the skin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalMetric {
    /// Temperature in Celsius or Fahrenheit
    Temperature,
    /// Maximum temperature recorded or critical threshold
    MaxTemperature,
    /// CPU Model or Sensor Name
    Name,
    /// Core / Sensor index temperature
    CoreTemp(usize),
}

/// Temperature scale format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureScale {
    Celsius,
    Fahrenheit,
    Kelvin,
}

/// Linux native Thermal & Hardware temperature measure scanning `/sys/class/hwmon` and `/sys/class/thermal`.
#[derive(Debug, Clone)]
pub struct ThermalMeasure {
    metric: ThermalMetric,
    scale: TemperatureScale,
    sensor_index: usize,
    hwmon_base: PathBuf,
    thermal_base: PathBuf,
    custom_name: Option<String>,
    mock_temp: Option<f64>,
    mock_name: Option<String>,
    current_value: MeasureValue,
}

impl ThermalMeasure {
    /// Create a new thermal measure for temperature in Celsius.
    pub fn new() -> Self {
        Self {
            metric: ThermalMetric::Temperature,
            scale: TemperatureScale::Celsius,
            sensor_index: 0,
            hwmon_base: PathBuf::from("/sys/class/hwmon"),
            thermal_base: PathBuf::from("/sys/class/thermal"),
            custom_name: None,
            mock_temp: None,
            mock_name: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Create with specific metric.
    pub fn with_metric(mut self, metric: ThermalMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Create with specific temperature scale.
    pub fn with_scale(mut self, scale: TemperatureScale) -> Self {
        self.scale = scale;
        self
    }

    /// Set custom hwmon and thermal directories (for testing).
    pub fn with_paths(mut self, hwmon_base: PathBuf, thermal_base: PathBuf) -> Self {
        self.hwmon_base = hwmon_base;
        self.thermal_base = thermal_base;
        self
    }

    /// Set mock temperature reading in Celsius.
    pub fn with_mock_temp(mut self, temp_c: f64) -> Self {
        self.mock_temp = Some(temp_c);
        self
    }

    /// Set mock sensor name.
    pub fn with_mock_name(mut self, name: &str) -> Self {
        self.mock_name = Some(name.to_string());
        self
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut measure = Self::new();

        // Check scale: Scale=C, Scale=F, TemperatureUnit=Fahrenheit, etc.
        if let Some(scale_str) = config.get("scale").or_else(|| config.get("temperatureunit")) {
            if scale_str.eq_ignore_ascii_case("f") || scale_str.eq_ignore_ascii_case("fahrenheit") {
                measure.scale = TemperatureScale::Fahrenheit;
            } else if scale_str.eq_ignore_ascii_case("k") || scale_str.eq_ignore_ascii_case("kelvin") {
                measure.scale = TemperatureScale::Kelvin;
            }
        }

        // Support CoreTemp plugin properties
        if let Some(coretemp_type) = config.get("coretemptype").or_else(|| config.get("type")) {
            let t_lower = coretemp_type.to_ascii_lowercase();
            match t_lower.as_str() {
                "cpuname" | "name" | "sensorname" => {
                    measure.metric = ThermalMetric::Name;
                }
                "maxtemperature" | "max" | "critical" => {
                    measure.metric = ThermalMetric::MaxTemperature;
                }
                "coretemp" | "core" => {
                    let idx = config
                        .get("coretempindex")
                        .or_else(|| config.get("index"))
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(0);
                    measure.metric = ThermalMetric::CoreTemp(idx);
                }
                _ => {
                    measure.metric = ThermalMetric::Temperature;
                }
            }
        }

        // Support SpeedFan plugin properties
        if let Some(speedfan_type) = config.get("speedfantype") {
            let s_lower = speedfan_type.to_ascii_lowercase();
            if s_lower == "temperature" || s_lower == "temp" {
                measure.metric = ThermalMetric::Temperature;
            }
        }

        if let Some(num) = config
            .get("speedfannumber")
            .or_else(|| config.get("sensorindex"))
            .or_else(|| config.get("index"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            measure.sensor_index = num;
            if matches!(measure.metric, ThermalMetric::Temperature) && num > 0 {
                measure.metric = ThermalMetric::CoreTemp(num);
            }
        }

        if let Some(sensor) = config.get("sensor").or_else(|| config.get("sensorname")) {
            measure.custom_name = Some(sensor.to_string());
        }

        measure
    }

    fn read_file_trimmed(path: &Path) -> Option<String> {
        let mut file = File::open(path).ok()?;
        let mut content = String::new();
        file.read_to_string(&mut content).ok()?;
        Some(content.trim().to_string())
    }

    fn read_temp_milli(path: &Path) -> Option<f64> {
        let val_str = Self::read_file_trimmed(path)?;
        val_str.parse::<f64>().ok().map(|m| m / 1000.0)
    }

    /// Discover CPU / system temperature from `/sys/class/hwmon` or `/sys/class/thermal`.
    fn query_system_temperature(&self) -> (f64, String) {
        if let Some(mock_c) = self.mock_temp {
            let name = self.mock_name.clone().unwrap_or_else(|| "CPU Package".to_string());
            return (mock_c, name);
        }

        // 1. Scan hwmon entries
        if self.hwmon_base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&self.hwmon_base) {
                let mut hwmon_dirs: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                hwmon_dirs.sort();

                // Look for preferred CPU drivers first (coretemp, k10temp, zenpower, asus-nb-wmi)
                let mut best_dir = None;
                let mut best_name = String::new();

                for dir in &hwmon_dirs {
                    let name_path = dir.join("name");
                    if let Some(name) = Self::read_file_trimmed(&name_path) {
                        let name_lower = name.to_ascii_lowercase();
                        if let Some(ref target) = self.custom_name {
                            if name_lower.contains(&target.to_ascii_lowercase()) {
                                best_dir = Some(dir.clone());
                                best_name = name;
                                break;
                            }
                        } else if name_lower.contains("coretemp")
                            || name_lower.contains("k10temp")
                            || name_lower.contains("zenpower")
                        {
                            best_dir = Some(dir.clone());
                            best_name = name;
                            break;
                        } else if best_dir.is_none() && (name_lower.contains("cpu") || name_lower.contains("asus") || name_lower.contains("thinkpad")) {
                            best_dir = Some(dir.clone());
                            best_name = name;
                        }
                    }
                }

                if best_dir.is_none() && !hwmon_dirs.is_empty() {
                    best_dir = Some(hwmon_dirs[0].clone());
                    best_name = Self::read_file_trimmed(&hwmon_dirs[0].join("name"))
                        .unwrap_or_else(|| "hwmon0".to_string());
                }

                if let Some(dir) = best_dir {
                    // Check for specific core index or package temperature
                    match self.metric {
                        ThermalMetric::CoreTemp(idx) => {
                            let temp_file = dir.join(format!("temp{}_input", idx + 1));
                            if let Some(temp) = Self::read_temp_milli(&temp_file) {
                                let label = Self::read_file_trimmed(&dir.join(format!("temp{}_label", idx + 1)))
                                    .unwrap_or_else(|| format!("Core {}", idx));
                                return (temp, label);
                            }
                        }
                        ThermalMetric::MaxTemperature => {
                            let mut max_t = 0.0f64;
                            for i in 1..=32 {
                                let temp_file = dir.join(format!("temp{}_input", i));
                                if let Some(temp) = Self::read_temp_milli(&temp_file) {
                                    if temp > max_t {
                                        max_t = temp;
                                    }
                                }
                            }
                            if max_t > 0.0 {
                                return (max_t, format!("{} Max", best_name));
                            }
                        }
                        _ => {
                            // Default package temperature (temp1_input)
                            for i in 1..=8 {
                                let temp_file = dir.join(format!("temp{}_input", i));
                                if let Some(temp) = Self::read_temp_milli(&temp_file) {
                                    let label = Self::read_file_trimmed(&dir.join(format!("temp{}_label", i)))
                                        .unwrap_or_else(|| best_name.clone());
                                    return (temp, label);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Fallback to /sys/class/thermal/thermal_zone*
        if self.thermal_base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&self.thermal_base) {
                let mut zones: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.starts_with("thermal_zone"))
                            .unwrap_or(false)
                    })
                    .collect();
                zones.sort();

                for zone in zones {
                    let temp_path = zone.join("temp");
                    let type_path = zone.join("type");
                    let zone_type = Self::read_file_trimmed(&type_path)
                        .unwrap_or_else(|| "thermal_zone".to_string());

                    if let Some(temp) = Self::read_temp_milli(&temp_path) {
                        return (temp, zone_type);
                    }
                }
            }
        }

        (40.0, "System Thermal".to_string())
    }

    fn convert_scale(&self, temp_c: f64) -> f64 {
        match self.scale {
            TemperatureScale::Celsius => temp_c,
            TemperatureScale::Fahrenheit => temp_c * 1.8 + 32.0,
            TemperatureScale::Kelvin => temp_c + 273.15,
        }
    }
}

impl Default for ThermalMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for ThermalMeasure {
    fn update(&mut self) -> MeasureValue {
        let (temp_c, name) = self.query_system_temperature();

        let val = match self.metric {
            ThermalMetric::Name => {
                if let Some(ref mock_name) = self.mock_name {
                    MeasureValue::String(mock_name.clone())
                } else {
                    MeasureValue::String(name)
                }
            }
            ThermalMetric::Temperature | ThermalMetric::MaxTemperature | ThermalMetric::CoreTemp(_) => {
                let scaled = self.convert_scale(temp_c);
                MeasureValue::Number(scaled)
            }
        };

        self.current_value = val;
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
