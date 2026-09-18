use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use std::fs;
use std::path::{Path, PathBuf};

/// Linux native Process plugin inspecting `/proc` for running process names.
#[derive(Debug, Clone)]
pub struct ProcessPlugin {
    process_name: String,
    proc_dir: PathBuf,
    current_value: MeasureValue,
}

impl ProcessPlugin {
    /// Create a new Process plugin checking for `process_name`.
    pub fn new(process_name: &str) -> Self {
        Self {
            process_name: process_name.to_string(),
            proc_dir: PathBuf::from("/proc"),
            current_value: MeasureValue::Number(-1.0),
        }
    }

    /// Instantiate from `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let name = config
            .get("processname")
            .or_else(|| config.get("process"))
            .or_else(|| config.get("name"))
            .unwrap_or("")
            .to_string();

        Self::new(&name)
    }

    /// Set custom `/proc` directory path (useful for deterministic mocking).
    pub fn with_proc_dir(mut self, path: PathBuf) -> Self {
        self.proc_dir = path;
        self
    }

    /// Set target process name.
    pub fn set_process_name(&mut self, name: &str) {
        self.process_name = name.to_string();
    }

    /// Returns true if a process matching `process_name` is active.
    pub fn is_running(&self) -> bool {
        if self.process_name.trim().is_empty() {
            return false;
        }

        let target_raw = self.process_name.trim().to_ascii_lowercase();
        let target_no_exe = target_raw
            .strip_suffix(".exe")
            .unwrap_or(&target_raw)
            .to_string();

        let entries = match fs::read_dir(&self.proc_dir) {
            Ok(e) => e,
            Err(_) => return false,
        };

        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            // Only check numeric PID directories
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let pid_dir = entry.path();

            // 1. Check comm file
            let comm_path = pid_dir.join("comm");
            if let Ok(comm_content) = fs::read_to_string(&comm_path) {
                let comm_trimmed = comm_content.trim().to_ascii_lowercase();
                let comm_no_exe = comm_trimmed
                    .strip_suffix(".exe")
                    .unwrap_or(&comm_trimmed);

                if comm_trimmed == target_raw
                    || comm_trimmed == target_no_exe
                    || comm_no_exe == target_no_exe
                {
                    return true;
                }
            }

            // 2. Check cmdline file
            let cmdline_path = pid_dir.join("cmdline");
            if let Ok(cmdline_bytes) = fs::read(&cmdline_path) {
                if let Some(first_arg) = cmdline_bytes.split(|&b| b == 0).next() {
                    let cmd_str = String::from_utf8_lossy(first_arg);
                    let path = Path::new(&*cmd_str);
                    if let Some(bin_name) = path.file_name().and_then(|f| f.to_str()) {
                        let bin_lower = bin_name.to_ascii_lowercase();
                        let bin_no_exe = bin_lower
                            .strip_suffix(".exe")
                            .unwrap_or(&bin_lower);

                        if bin_lower == target_raw
                            || bin_lower == target_no_exe
                            || bin_no_exe == target_no_exe
                        {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}

impl Measure for ProcessPlugin {
    fn update(&mut self) -> MeasureValue {
        let running = self.is_running();
        self.current_value = MeasureValue::Number(if running { 1.0 } else { -1.0 });
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
