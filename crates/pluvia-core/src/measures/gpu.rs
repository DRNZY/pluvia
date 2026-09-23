use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Target GPU property to extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuMetric {
    /// GPU core utilization percentage (0 to 100)
    Usage,
    /// GPU VRAM used in bytes
    MemoryUsed,
    /// GPU VRAM total in bytes
    MemoryTotal,
    /// GPU VRAM used percentage (0 to 100)
    MemoryPercent,
    /// GPU temperature in Celsius
    Temperature,
    /// GPU core clock frequency in MHz
    Clock,
    /// GPU hardware name / model string
    Name,
}

/// Linux native GPU telemetry measure supporting NVIDIA, AMD, and Intel GPUs.
#[derive(Debug, Clone)]
pub struct GpuMeasure {
    metric: GpuMetric,
    drm_base: PathBuf,
    gpu_index: usize,
    mock_usage: Option<f64>,
    mock_memory_used: Option<f64>,
    mock_memory_total: Option<f64>,
    mock_temp: Option<f64>,
    mock_clock: Option<f64>,
    mock_name: Option<String>,
    current_value: MeasureValue,
}

impl GpuMeasure {
    /// Create a new GPU measure for utilization percentage.
    pub fn new() -> Self {
        Self {
            metric: GpuMetric::Usage,
            drm_base: PathBuf::from("/sys/class/drm"),
            gpu_index: 0,
            mock_usage: None,
            mock_memory_used: None,
            mock_memory_total: None,
            mock_temp: None,
            mock_clock: None,
            mock_name: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Create with specific GPU metric.
    pub fn with_metric(mut self, metric: GpuMetric) -> Self {
        self.metric = metric;
        self
    }

    /// Set mock GPU telemetry values for tests.
    pub fn with_mock(
        mut self,
        usage: f64,
        mem_used_mb: f64,
        mem_total_mb: f64,
        temp_c: f64,
        name: &str,
    ) -> Self {
        self.mock_usage = Some(usage.clamp(0.0, 100.0));
        self.mock_memory_used = Some(mem_used_mb * 1024.0 * 1024.0);
        self.mock_memory_total = Some(mem_total_mb * 1024.0 * 1024.0);
        self.mock_temp = Some(temp_c);
        self.mock_name = Some(name.to_string());
        self
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let mut measure = Self::new();

        let type_str = config
            .get("gputype")
            .or_else(|| config.get("type"))
            .or_else(|| config.get("counter"))
            .or_else(|| config.get("entry"))
            .unwrap_or("usage")
            .to_ascii_lowercase();

        measure.metric = match type_str.as_str() {
            "vram" | "memory" | "memused" | "gpu memory" => GpuMetric::MemoryUsed,
            "vramtotal" | "memorytotal" | "memtotal" => GpuMetric::MemoryTotal,
            "vrampercent" | "memorypercent" | "mempercent" | "gpu memory usage" => GpuMetric::MemoryPercent,
            "temp" | "temperature" | "gputemp" | "gpu temperature" => GpuMetric::Temperature,
            "clock" | "gpuclock" | "coreclock" => GpuMetric::Clock,
            "name" | "gpuname" | "model" => GpuMetric::Name,
            _ => GpuMetric::Usage,
        };

        if let Some(idx) = config
            .get("gpuindex")
            .or_else(|| config.get("index"))
            .and_then(|s| s.parse::<usize>().ok())
        {
            measure.gpu_index = idx;
        }

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

    fn query_gpu_stats(&self) -> (f64, f64, f64, f64, f64, String) {
        if let (Some(u), Some(mu), Some(mt), Some(t), Some(ref n)) = (
            self.mock_usage,
            self.mock_memory_used,
            self.mock_memory_total,
            self.mock_temp,
            &self.mock_name,
        ) {
            let clock = self.mock_clock.unwrap_or(1500.0);
            return (u, mu, mt, t, clock, n.clone());
        }

        // 1. Check NVIDIA GPU via nvidia-smi
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=utilization.gpu,temperature.gpu,memory.used,memory.total,clocks.current.graphics,name",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = stdout.lines().collect();
                if let Some(line) = lines.get(self.gpu_index).or_else(|| lines.first()) {
                    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
                    if parts.len() >= 6 {
                        let usage = parts[0].parse::<f64>().unwrap_or(0.0);
                        let temp = parts[1].parse::<f64>().unwrap_or(0.0);
                        let mem_used_mb = parts[2].parse::<f64>().unwrap_or(0.0);
                        let mem_total_mb = parts[3].parse::<f64>().unwrap_or(0.0);
                        let clock_mhz = parts[4].parse::<f64>().unwrap_or(0.0);
                        let name = parts[5].to_string();

                        return (
                            usage,
                            mem_used_mb * 1024.0 * 1024.0,
                            mem_total_mb * 1024.0 * 1024.0,
                            temp,
                            clock_mhz,
                            name,
                        );
                    }
                }
            }
        }

        // 2. Check AMD / Intel DRM sysfs in /sys/class/drm/card*
        if self.drm_base.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&self.drm_base) {
                let mut cards: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.starts_with("card") && !s.contains('-'))
                            .unwrap_or(false)
                    })
                    .collect();
                cards.sort();

                if let Some(card) = cards.get(self.gpu_index).or_else(|| cards.first()) {
                    let dev = card.join("device");
                    let mut usage = 0.0f64;
                    let mut mem_used = 0.0f64;
                    let mut mem_total = 0.0f64;
                    let mut temp = 45.0f64;
                    let mut clock = 0.0f64;
                    let mut name = "Integrated GPU".to_string();

                    // AMD gpu_busy_percent
                    if let Some(pct) = Self::read_file_u64(&dev.join("gpu_busy_percent")) {
                        usage = pct as f64;
                        name = "AMD Radeon Graphics".to_string();
                    }

                    // AMD VRAM stats
                    if let Some(vram_used) = Self::read_file_u64(&dev.join("mem_info_vram_used")) {
                        mem_used = vram_used as f64;
                    }
                    if let Some(vram_total) = Self::read_file_u64(&dev.join("mem_info_vram_total")) {
                        mem_total = vram_total as f64;
                    }

                    // AMD / Intel GPU temperature
                    let hwmon_dir = dev.join("hwmon");
                    if hwmon_dir.is_dir() {
                        if let Ok(h_entries) = std::fs::read_dir(&hwmon_dir) {
                            for h in h_entries.flatten() {
                                if let Some(t_milli) = Self::read_file_u64(&h.path().join("temp1_input")) {
                                    temp = t_milli as f64 / 1000.0;
                                    break;
                                }
                            }
                        }
                    }

                    // Intel actual frequency
                    if let Some(freq_mhz) = Self::read_file_u64(&card.join("gt_act_freq_mhz")) {
                        clock = freq_mhz as f64;
                        name = "Intel HD/Iris Graphics".to_string();
                    }

                    return (usage, mem_used, mem_total, temp, clock, name);
                }
            }
        }

        (0.0, 0.0, 0.0, 35.0, 0.0, "Generic GPU".to_string())
    }
}

impl Default for GpuMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for GpuMeasure {
    fn update(&mut self) -> MeasureValue {
        let (usage, mem_used, mem_total, temp, clock, name) = self.query_gpu_stats();

        let val = match self.metric {
            GpuMetric::Usage => MeasureValue::Number(usage.clamp(0.0, 100.0)),
            GpuMetric::MemoryUsed => MeasureValue::Number(mem_used),
            GpuMetric::MemoryTotal => MeasureValue::Number(mem_total),
            GpuMetric::MemoryPercent => {
                let pct = if mem_total > 0.0 {
                    (mem_used / mem_total) * 100.0
                } else {
                    0.0
                };
                MeasureValue::Number(pct.clamp(0.0, 100.0))
            }
            GpuMetric::Temperature => MeasureValue::Number(temp),
            GpuMetric::Clock => MeasureValue::Number(clock),
            GpuMetric::Name => MeasureValue::String(name),
        };

        self.current_value = val;
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
