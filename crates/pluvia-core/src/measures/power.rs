use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Power state metric to measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerMetric {
    /// Battery charge level percentage (0 to 100)
    Percent,
    /// Power status code: 0 = Battery, 1 = Charging, 2 = Critical, 3 = High/Full
    Status,
    /// AC line status: 0 = Offline/Battery, 1 = Online/AC Connected
    ACLine,
    /// Estimated battery lifetime remaining in seconds (-1 if on AC or unknown)
    Lifetime,
    /// Current CPU operating frequency in Hz / MHz
    Hz,
    /// Human-readable battery state ("Charging", "Discharging", "Full")
    StateText,
}

/// Linux native Battery & Power management measure scanning `/sys/class/power_supply`.
#[derive(Debug, Clone)]
pub struct PowerMeasure {
    metric: PowerMetric,
    power_supply_base: PathBuf,
    cpu_freq_path: PathBuf,
    mock_percent: Option<f64>,
    mock_ac: Option<bool>,
    mock_status: Option<String>,
    mock_lifetime_secs: Option<f64>,
    mock_hz: Option<f64>,
    current_value: MeasureValue,
}

impl PowerMeasure {
    /// Create a new power measure for battery percentage.
    pub fn new() -> Self {
        Self {
            metric: PowerMetric::Percent,
            power_supply_base: PathBuf::from("/sys/class/power_supply"),
            cpu_freq_path: PathBuf::from("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"),
            mock_percent: None,
            mock_ac: None,
            mock_status: None,
            mock_lifetime_secs: None,
            mock_hz: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Create with specific power metric.
    pub fn with_metric(mut self, metric: PowerMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Set custom power supply directory for testing.
    pub fn with_base(mut self, path: PathBuf) -> Self {
        self.power_supply_base = path;
        self
    }

    /// Set mock battery percentage.
    pub fn with_mock_percent(mut self, percent: f64) -> Self {
        self.mock_percent = Some(percent.clamp(0.0, 100.0));
        self
    }

    /// Set mock AC line status.
    pub fn with_mock_ac(mut self, ac: bool) -> Self {
        self.mock_ac = Some(ac);
        self
    }

    /// Set mock battery status text ("Charging", "Discharging", "Full").
    pub fn with_mock_status(mut self, status: &str) -> Self {
        self.mock_status = Some(status.to_string());
        self
    }

    /// Set mock lifetime in seconds.
    pub fn with_mock_lifetime(mut self, secs: f64) -> Self {
        self.mock_lifetime_secs = Some(secs);
        self
    }

    /// Set mock CPU frequency in MHz.
    pub fn with_mock_hz(mut self, mhz: f64) -> Self {
        self.mock_hz = Some(mhz);
        self
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut measure = Self::new();

        let state_str = config
            .get("powerstate")
            .or_else(|| config.get("type"))
            .or_else(|| config.get("metric"))
            .unwrap_or("percent")
            .to_ascii_lowercase();

        measure.metric = match state_str.as_str() {
            "status" | "state" => PowerMetric::Status,
            "statustext" | "statusstring" | "string" => PowerMetric::StateText,
            "acline" | "ac" | "online" => PowerMetric::ACLine,
            "lifetime" | "remaining" | "timeremaining" => PowerMetric::Lifetime,
            "hz" | "cpufreq" | "freq" | "frequency" => PowerMetric::Hz,
            _ => PowerMetric::Percent,
        };

        measure
    }

    fn read_file_trimmed(path: &Path) -> Option<String> {
        let mut file = File::open(path).ok()?;
        let mut content = String::new();
        file.read_to_string(&mut content).ok()?;
        Some(content.trim().to_string())
    }

    fn read_file_u64(path: &Path) -> Option<u64> {
        Self::read_file_trimmed(path)?.parse::<u64>().ok()
    }

    fn query_power_supply(&self) -> (f64, bool, String, f64, f64) {
        if let (Some(pct), Some(ac)) = (self.mock_percent, self.mock_ac) {
            let status = self.mock_status.clone().unwrap_or_else(|| {
                if ac { "Charging".to_string() } else { "Discharging".to_string() }
            });
            let lifetime = self.mock_lifetime_secs.unwrap_or(3600.0);
            let hz = self.mock_hz.unwrap_or(2400.0);
            return (pct, ac, status, lifetime, hz);
        }

        let mut battery_capacity: Option<f64> = self.mock_percent;
        let mut ac_online: Option<bool> = self.mock_ac;
        let mut status_str = self.mock_status.clone().unwrap_or_else(|| "Unknown".to_string());
        let mut lifetime_secs = self.mock_lifetime_secs.unwrap_or(-1.0);
        let mut cpu_hz = self.mock_hz.unwrap_or(0.0);

        if self.power_supply_base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&self.power_supply_base) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Check AC adapters
                    if name.starts_with("AC") || name.starts_with("ADP") || name.starts_with("ucsi") {
                        if let Some(online) = Self::read_file_u64(&path.join("online")) {
                            if online == 1 {
                                ac_online = Some(true);
                            } else if ac_online.is_none() {
                                ac_online = Some(false);
                            }
                        }
                    }

                    // Check Batteries (BAT0, BAT1, etc.)
                    if name.starts_with("BAT") {
                        if let Some(cap) = Self::read_file_u64(&path.join("capacity")) {
                            battery_capacity = Some(cap as f64);
                        } else if let (Some(now), Some(full)) = (
                            Self::read_file_u64(&path.join("energy_now"))
                                .or_else(|| Self::read_file_u64(&path.join("charge_now"))),
                            Self::read_file_u64(&path.join("energy_full"))
                                .or_else(|| Self::read_file_u64(&path.join("charge_full"))),
                        ) {
                            if full > 0 {
                                battery_capacity = Some((now as f64 / full as f64) * 100.0);
                            }
                        }

                        if let Some(st) = Self::read_file_trimmed(&path.join("status")) {
                            status_str = st;
                        }

                        // Calculate lifetime if discharging
                        if let (Some(energy_now), Some(power_now)) = (
                            Self::read_file_u64(&path.join("energy_now")),
                            Self::read_file_u64(&path.join("power_now")),
                        ) {
                            if power_now > 0 {
                                lifetime_secs = (energy_now as f64 / power_now as f64) * 3600.0;
                            }
                        } else if let (Some(charge_now), Some(current_now)) = (
                            Self::read_file_u64(&path.join("charge_now")),
                            Self::read_file_u64(&path.join("current_now")),
                        ) {
                            if current_now > 0 {
                                lifetime_secs = (charge_now as f64 / current_now as f64) * 3600.0;
                            }
                        }
                    }
                }
            }
        }

        // Query CPU frequency for Hz metric
        if cpu_hz <= 0.0 {
            if let Some(khz) = Self::read_file_u64(&self.cpu_freq_path) {
                cpu_hz = (khz as f64) / 1000.0; // MHz
            } else if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
                for line in cpuinfo.lines() {
                    if line.starts_with("cpu MHz") {
                        if let Some((_, mhz_str)) = line.split_once(':') {
                            if let Ok(mhz) = mhz_str.trim().parse::<f64>() {
                                cpu_hz = mhz;
                                break;
                            }
                        }
                    }
                }
            }
        }

        let cap = battery_capacity.unwrap_or(100.0);
        let ac = ac_online.unwrap_or(true);

        (cap, ac, status_str, lifetime_secs, cpu_hz)
    }
}

impl Default for PowerMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for PowerMeasure {
    fn update(&mut self) -> MeasureValue {
        let (cap, ac, status_str, lifetime_secs, cpu_hz) = self.query_power_supply();

        let val = match self.metric {
            PowerMetric::Percent => MeasureValue::Number(cap.clamp(0.0, 100.0)),
            PowerMetric::ACLine => MeasureValue::Number(if ac { 1.0 } else { 0.0 }),
            PowerMetric::Lifetime => MeasureValue::Number(lifetime_secs.max(-1.0)),
            PowerMetric::Hz => MeasureValue::Number(cpu_hz),
            PowerMetric::StateText => MeasureValue::String(status_str.clone()),
            PowerMetric::Status => {
                let st_lower = status_str.to_ascii_lowercase();
                let code = if st_lower.contains("charging") {
                    1.0
                } else if cap <= 10.0 {
                    2.0
                } else if cap >= 95.0 {
                    3.0
                } else {
                    0.0
                };
                MeasureValue::Number(code)
            }
        };

        self.current_value = val;
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
