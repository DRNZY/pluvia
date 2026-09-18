use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::ffi::CString;
use std::fs::File;
use std::io::Read;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

fn parse_cpu_times(line: &str) -> CpuTimes {
    let mut parts = line.split_whitespace().skip(1);
    let user = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let nice = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let system = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let idle = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let iowait = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let irq = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let softirq = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let steal = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

    CpuTimes {
        user,
        nice,
        system,
        idle,
        iowait,
        irq,
        softirq,
        steal,
    }
}

/// Zero-allocation delta CPU usage measure parsing `/proc/stat`.
#[derive(Debug)]
pub struct CpuMeasure {
    stat_path: PathBuf,
    processor: Option<usize>,
    prev_user: u64,
    prev_nice: u64,
    prev_system: u64,
    prev_idle: u64,
    prev_iowait: u64,
    prev_irq: u64,
    prev_softirq: u64,
    prev_steal: u64,
    has_prev: bool,
    buffer: String,
    current_value: MeasureValue,
}

impl CpuMeasure {
    /// Create a measure for overall CPU usage using `/proc/stat`.
    pub fn new() -> Self {
        Self::with_path(PathBuf::from("/proc/stat"), None)
    }

    /// Create a measure for a specific processor core (0 = overall, 1 = core 0, etc.).
    pub fn with_processor(processor: Option<usize>) -> Self {
        Self::with_path(PathBuf::from("/proc/stat"), processor)
    }

    /// Create a CPU measure with a custom `/proc/stat` path (for testing or container environments).
    pub fn with_path(stat_path: PathBuf, processor: Option<usize>) -> Self {
        Self {
            stat_path,
            processor,
            prev_user: 0,
            prev_nice: 0,
            prev_system: 0,
            prev_idle: 0,
            prev_iowait: 0,
            prev_irq: 0,
            prev_softirq: 0,
            prev_steal: 0,
            has_prev: false,
            buffer: String::with_capacity(2048),
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Update the path to `/proc/stat`.
    pub fn set_stat_path(&mut self, path: PathBuf) {
        self.stat_path = path;
    }

    /// Instantiate from a Rainmeter `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let processor = config
            .get("processor")
            .and_then(|s| s.parse::<usize>().ok());
        Self::with_processor(processor)
    }

    fn record_and_calculate_delta(&mut self, times: CpuTimes) -> MeasureValue {
        let curr_idle = times.idle + times.iowait;
        let curr_total = times.user
            + times.nice
            + times.system
            + times.idle
            + times.iowait
            + times.irq
            + times.softirq
            + times.steal;

        let prev_idle = self.prev_idle + self.prev_iowait;
        let prev_total = self.prev_user
            + self.prev_nice
            + self.prev_system
            + self.prev_idle
            + self.prev_iowait
            + self.prev_irq
            + self.prev_softirq
            + self.prev_steal;

        self.prev_user = times.user;
        self.prev_nice = times.nice;
        self.prev_system = times.system;
        self.prev_idle = times.idle;
        self.prev_iowait = times.iowait;
        self.prev_irq = times.irq;
        self.prev_softirq = times.softirq;
        self.prev_steal = times.steal;

        if !self.has_prev {
            self.has_prev = true;
            self.current_value = MeasureValue::Number(0.0);
            return self.current_value.clone();
        }

        let delta_total = curr_total.saturating_sub(prev_total);
        let delta_idle = curr_idle.saturating_sub(prev_idle);
        let delta_work = delta_total.saturating_sub(delta_idle);

        let pct = if delta_total > 0 {
            (delta_work as f64 / delta_total as f64) * 100.0
        } else {
            0.0
        };

        self.current_value = MeasureValue::Number(pct);
        self.current_value.clone()
    }
}

impl Default for CpuMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for CpuMeasure {
    fn update(&mut self) -> MeasureValue {
        self.buffer.clear();
        let mut file = match File::open(&self.stat_path) {
            Ok(f) => f,
            Err(_) => return self.current_value.clone(),
        };

        if file.read_to_string(&mut self.buffer).is_err() {
            return self.current_value.clone();
        }

        let found_times = match self.processor {
            None | Some(0) => {
                let mut found = None;
                for line in self.buffer.lines() {
                    if line.starts_with("cpu ") {
                        found = Some(parse_cpu_times(line));
                        break;
                    }
                }
                found
            }
            Some(n) => {
                let core_idx = n.saturating_sub(1);
                let mut found = None;
                for line in self.buffer.lines() {
                    if let Some(rest) = line.strip_prefix("cpu") {
                        let mut digits = "";
                        for (idx, ch) in rest.char_indices() {
                            if ch == ' ' {
                                digits = &rest[..idx];
                                break;
                            }
                        }
                        if let Ok(idx) = digits.parse::<usize>() {
                            if idx == core_idx {
                                found = Some(parse_cpu_times(line));
                                break;
                            }
                        }
                    }
                }
                found
            }
        };

        if let Some(times) = found_times {
            self.record_and_calculate_delta(times)
        } else {
            MeasureValue::Number(0.0)
        }
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}

/// Linux memory measure parsing `/proc/meminfo`.
#[derive(Debug)]
pub struct MemoryMeasure {
    meminfo_path: PathBuf,
    metric: String,
    buffer: String,
    current_value: MeasureValue,
}

impl MemoryMeasure {
    /// Create a memory measure for the specified metric (e.g. `"used_percent"`, `"total"`, `"used"`, `"free"`).
    pub fn new(metric: &str) -> Self {
        Self::with_path(PathBuf::from("/proc/meminfo"), metric)
    }

    /// Create with custom `/proc/meminfo` path.
    pub fn with_path(meminfo_path: PathBuf, metric: &str) -> Self {
        Self {
            meminfo_path,
            metric: metric.to_ascii_lowercase(),
            buffer: String::with_capacity(2048),
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Set path to `/proc/meminfo`.
    pub fn set_meminfo_path(&mut self, path: PathBuf) {
        self.meminfo_path = path;
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let is_swap = config.measure_type.eq_ignore_ascii_case("swapmemory");
        let is_total = config.get("total").map(|s| s == "1").unwrap_or(false);
        let is_percent = config
            .get("percent")
            .or_else(|| config.get("percentual"))
            .map(|s| s == "1")
            .unwrap_or(false);

        let metric = if is_swap {
            if is_total {
                "swap_total"
            } else if is_percent {
                "swap_percent"
            } else {
                "swap_used"
            }
        } else if is_total {
            "total"
        } else if is_percent {
            "used_percent"
        } else {
            "used"
        };

        Self::new(metric)
    }
}

impl Measure for MemoryMeasure {
    fn update(&mut self) -> MeasureValue {
        self.buffer.clear();
        let mut file = match File::open(&self.meminfo_path) {
            Ok(f) => f,
            Err(_) => return self.current_value.clone(),
        };

        if file.read_to_string(&mut self.buffer).is_err() {
            return self.current_value.clone();
        }

        let mut mem_total: u64 = 0;
        let mut mem_free: u64 = 0;
        let mut mem_available: u64 = 0;
        let mut buffers: u64 = 0;
        let mut cached: u64 = 0;
        let mut swap_total: u64 = 0;
        let mut swap_free: u64 = 0;

        for line in self.buffer.lines() {
            if let Some((k, v)) = line.split_once(':') {
                let key = k.trim();
                let val_kb = v
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                match key {
                    "MemTotal" => mem_total = val_kb * 1024,
                    "MemFree" => mem_free = val_kb * 1024,
                    "MemAvailable" => mem_available = val_kb * 1024,
                    "Buffers" => buffers = val_kb * 1024,
                    "Cached" => cached = val_kb * 1024,
                    "SwapTotal" => swap_total = val_kb * 1024,
                    "SwapFree" => swap_free = val_kb * 1024,
                    _ => {}
                }
            }
        }

        let free_bytes = if mem_available > 0 {
            mem_available
        } else {
            mem_free + buffers + cached
        };
        let used_bytes = mem_total.saturating_sub(free_bytes);
        let used_percent = if mem_total > 0 {
            (used_bytes as f64 / mem_total as f64) * 100.0
        } else {
            0.0
        };

        let swap_used_bytes = swap_total.saturating_sub(swap_free);
        let swap_percent = if swap_total > 0 {
            (swap_used_bytes as f64 / swap_total as f64) * 100.0
        } else {
            0.0
        };

        let val = match self.metric.as_str() {
            "used_percent" | "percent" => used_percent,
            "total" | "total_bytes" => mem_total as f64,
            "used" | "used_bytes" => used_bytes as f64,
            "free" | "free_bytes" => free_bytes as f64,
            "free_percent" => (100.0 - used_percent).max(0.0),
            "swap_total" => swap_total as f64,
            "swap_used" => swap_used_bytes as f64,
            "swap_free" => swap_free as f64,
            "swap_percent" => swap_percent,
            _ => used_percent,
        };

        self.current_value = MeasureValue::Number(val);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}

/// Linux disk space measure using `statvfs`.
#[derive(Debug, Clone)]
pub struct DiskMeasure {
    mount_path: String,
    metric: String,
    current_value: MeasureValue,
}

impl DiskMeasure {
    /// Create a disk measure for a given mount point and metric (`"free"`, `"total"`, `"used"`, `"percent"`).
    pub fn new(mount_path: &str, metric: &str) -> Self {
        Self {
            mount_path: mount_path.to_string(),
            metric: metric.to_ascii_lowercase(),
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let drive = config
            .get("drive")
            .unwrap_or("/");

        // Normalize Windows drive letters (C:, C) to Linux root "/"
        let mount_path = if drive.is_empty() || drive.eq_ignore_ascii_case("c:") || drive.eq_ignore_ascii_case("c") {
            "/"
        } else {
            drive
        };

        let is_total = config.get("total").map(|s| s == "1").unwrap_or(false);
        let is_used = config.get("invertmeasure").map(|s| s == "1").unwrap_or(false);
        let is_percent = config
            .get("percent")
            .or_else(|| config.get("percentual"))
            .map(|s| s == "1")
            .unwrap_or(false);

        let metric = if is_total {
            "total"
        } else if is_percent {
            "percent"
        } else if is_used {
            "used"
        } else {
            "free"
        };

        Self::new(mount_path, metric)
    }
}

impl Measure for DiskMeasure {
    fn update(&mut self) -> MeasureValue {
        let c_path = match CString::new(self.mount_path.as_str()) {
            Ok(p) => p,
            Err(_) => return self.current_value.clone(),
        };

        let mut stat = MaybeUninit::<libc::statvfs>::uninit();
        let res = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
        if res != 0 {
            return self.current_value.clone();
        }

        let stat = unsafe { stat.assume_init() };
        let block_size = if stat.f_frsize > 0 {
            stat.f_frsize
        } else {
            stat.f_bsize
        } as u64;

        let total_bytes = stat.f_blocks as u64 * block_size;
        let free_bytes = stat.f_bavail as u64 * block_size;
        let used_bytes = total_bytes.saturating_sub(free_bytes);
        let used_percent = if total_bytes > 0 {
            (used_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        let val = match self.metric.as_str() {
            "total" | "total_bytes" => total_bytes as f64,
            "free" | "free_bytes" => free_bytes as f64,
            "used" | "used_bytes" => used_bytes as f64,
            "percent" | "used_percent" => used_percent,
            "free_percent" => (100.0 - used_percent).max(0.0),
            _ => free_bytes as f64,
        };

        self.current_value = MeasureValue::Number(val);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}

/// Linux system uptime measure reading `/proc/uptime`.
#[derive(Debug, Clone)]
pub struct UptimeMeasure {
    uptime_path: PathBuf,
    format: Option<String>,
    current_value: MeasureValue,
}

impl UptimeMeasure {
    /// Create an uptime measure reading `/proc/uptime`.
    pub fn new() -> Self {
        Self {
            uptime_path: PathBuf::from("/proc/uptime"),
            format: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Set format string.
    pub fn with_format(mut self, format: &str) -> Self {
        self.format = Some(format.to_string());
        self
    }

    /// Set path to `/proc/uptime`.
    pub fn with_path(path: PathBuf) -> Self {
        Self {
            uptime_path: path,
            format: None,
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let format = config.format.clone().or_else(|| config.get("format").map(|s| s.to_string()));
        Self {
            uptime_path: PathBuf::from("/proc/uptime"),
            format,
            current_value: MeasureValue::Number(0.0),
        }
    }
}

impl Default for UptimeMeasure {
    fn default() -> Self {
        Self::new()
    }
}

impl Measure for UptimeMeasure {
    fn update(&mut self) -> MeasureValue {
        let content = match std::fs::read_to_string(&self.uptime_path) {
            Ok(c) => c,
            Err(_) => return self.current_value.clone(),
        };

        let uptime_secs: f64 = content
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        if let Some(ref fmt) = self.format {
            let total_secs = uptime_secs.floor() as u64;
            let days = total_secs / 86400;
            let hours = (total_secs % 86400) / 3600;
            let mins = (total_secs % 3600) / 60;
            let secs = total_secs % 60;

            // Handle Rainmeter format specifiers: %4 = days, %3 = hours, %2 = mins, %1 = secs
            let mut formatted = fmt.clone();
            formatted = formatted.replace("%4!02d!", &format!("{:02}", days));
            formatted = formatted.replace("%4!d!", &format!("{}", days));
            formatted = formatted.replace("%3!02d!", &format!("{:02}", hours));
            formatted = formatted.replace("%3!d!", &format!("{}", hours));
            formatted = formatted.replace("%2!02d!", &format!("{:02}", mins));
            formatted = formatted.replace("%2!d!", &format!("{}", mins));
            formatted = formatted.replace("%1!02d!", &format!("{:02}", secs));
            formatted = formatted.replace("%1!d!", &format!("{}", secs));

            self.current_value = MeasureValue::String(formatted);
        } else {
            self.current_value = MeasureValue::Number(uptime_secs);
        }

        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}

/// Type of network bandwidth measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetMeasureType {
    In,
    Out,
    Total,
}

/// Linux network bandwidth delta measure reading `/proc/net/dev`.
#[derive(Debug)]
pub struct NetMeasure {
    dev_path: PathBuf,
    net_type: NetMeasureType,
    interface: Option<String>,
    prev_rx_bytes: u64,
    prev_tx_bytes: u64,
    prev_time: Option<Instant>,
    buffer: String,
    current_value: MeasureValue,
}

impl NetMeasure {
    /// Create a new network measure for In, Out, or Total traffic.
    pub fn new(net_type: NetMeasureType) -> Self {
        Self::with_path(PathBuf::from("/proc/net/dev"), net_type, None)
    }

    /// Create with specific interface (e.g. `"eth0"` or `"wlan0"`).
    pub fn with_interface(net_type: NetMeasureType, interface: Option<String>) -> Self {
        Self::with_path(PathBuf::from("/proc/net/dev"), net_type, interface)
    }

    /// Create with custom path and interface.
    pub fn with_path(
        dev_path: PathBuf,
        net_type: NetMeasureType,
        interface: Option<String>,
    ) -> Self {
        Self {
            dev_path,
            net_type,
            interface,
            prev_rx_bytes: 0,
            prev_tx_bytes: 0,
            prev_time: None,
            buffer: String::with_capacity(2048),
            current_value: MeasureValue::Number(0.0),
        }
    }

    /// Set path to `/proc/net/dev`.
    pub fn set_dev_path(&mut self, path: PathBuf) {
        self.dev_path = path;
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let m_type = config.measure_type.to_ascii_lowercase();
        let net_type = match m_type.as_str() {
            "netin" => NetMeasureType::In,
            "netout" => NetMeasureType::Out,
            _ => NetMeasureType::Total,
        };

        let interface = config.get("interface").map(|s| s.to_string());
        Self::with_interface(net_type, interface)
    }
}

impl Measure for NetMeasure {
    fn update(&mut self) -> MeasureValue {
        self.buffer.clear();
        let mut file = match File::open(&self.dev_path) {
            Ok(f) => f,
            Err(_) => return self.current_value.clone(),
        };

        if file.read_to_string(&mut self.buffer).is_err() {
            return self.current_value.clone();
        }

        let mut total_rx: u64 = 0;
        let mut total_tx: u64 = 0;

        for line in self.buffer.lines().skip(2) {
            if let Some((iface, stats)) = line.split_once(':') {
                let iface_name = iface.trim();

                // If specific interface requested, filter on it
                if let Some(ref target) = self.interface {
                    if !target.eq_ignore_ascii_case("all")
                        && !target.eq_ignore_ascii_case("0")
                        && !iface_name.eq_ignore_ascii_case(target)
                    {
                        continue;
                    }
                } else if iface_name == "lo" {
                    // Default to ignoring loopback interface
                    continue;
                }

                let mut fields = stats.split_whitespace();
                let rx_bytes = fields.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                // Transmit bytes is field index 8 (9th field in numbers)
                let tx_bytes = fields.nth(7).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);

                total_rx += rx_bytes;
                total_tx += tx_bytes;
            }
        }

        let now = Instant::now();
        if let Some(prev_instant) = self.prev_time {
            let elapsed_secs = now.duration_since(prev_instant).as_secs_f64();
            let delta_rx = total_rx.saturating_sub(self.prev_rx_bytes);
            let delta_tx = total_tx.saturating_sub(self.prev_tx_bytes);

            let delta_bytes = match self.net_type {
                NetMeasureType::In => delta_rx,
                NetMeasureType::Out => delta_tx,
                NetMeasureType::Total => delta_rx + delta_tx,
            };

            let rate = if elapsed_secs > 0.0 {
                delta_bytes as f64 / elapsed_secs
            } else {
                delta_bytes as f64
            };

            self.prev_rx_bytes = total_rx;
            self.prev_tx_bytes = total_tx;
            self.prev_time = Some(now);
            self.current_value = MeasureValue::Number(rate);
        } else {
            self.prev_rx_bytes = total_rx;
            self.prev_tx_bytes = total_tx;
            self.prev_time = Some(now);
            self.current_value = MeasureValue::Number(0.0);
        }

        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
