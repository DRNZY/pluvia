use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub status: String,
    pub uptime_secs: u64,
    pub active_skins: Vec<String>,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinInfo {
    pub id: String,
    pub path: String,
    pub update_rate_ms: u64,
    pub measures_count: usize,
    pub meters_count: usize,
    pub bounds: SurfaceBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SurfaceBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct IpcClient {
    socket_path: PathBuf,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            socket_path: default_socket_path(),
        }
    }

    pub fn with_socket<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket_path: path.as_ref().to_path_buf(),
        }
    }

    pub fn is_daemon_running(&self) -> bool {
        self.ping().is_ok()
    }

    /// Automatically starts the pluvia-daemon background engine if not already running.
    pub fn start_daemon(&self) -> Result<(), String> {
        if self.is_daemon_running() {
            return Ok(());
        }

        // Clean up stale socket file if daemon isn't alive
        let _ = std::fs::remove_file(&self.socket_path);

        let bin = find_daemon_binary();
        let mut cmd = std::process::Command::new(&bin);
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        let _ = cmd.spawn().map_err(|e| format!("Failed to start daemon from {:?}: {}", bin, e))?;

        // Wait up to 2.5 seconds for daemon to initialize
        for _ in 0..25 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if self.is_daemon_running() {
                return Ok(());
            }
        }

        if self.is_daemon_running() {
            Ok(())
        } else {
            Err("Engine launched but socket connection timed out".to_string())
        }
    }

    /// Stops the pluvia-daemon process.
    pub fn stop_daemon(&self) -> Result<(), String> {
        let _ = std::process::Command::new("pkill")
            .arg("-f")
            .arg("pluvia-daemon")
            .output();
        let _ = std::fs::remove_file(&self.socket_path);
        Ok(())
    }

    pub fn ping(&self) -> Result<String, String> {
        let resp = self.call("pluvia.ping", json!({}))?;
        resp.as_str()
            .map(str::to_string)
            .ok_or_else(|| "Invalid ping response".to_string())
    }

    pub fn get_state(&self) -> Result<DaemonState, String> {
        let resp = self.call("pluvia.getState", json!({}))?;
        serde_json::from_value(resp).map_err(|e| e.to_string())
    }

    pub fn list_skins(&self) -> Result<Vec<SkinInfo>, String> {
        let resp = self.call("pluvia.listSkins", json!({}))?;
        serde_json::from_value(resp).map_err(|e| e.to_string())
    }

    pub fn load_skin<P: AsRef<Path>>(&self, path: P) -> Result<SkinInfo, String> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let resp = self.call("pluvia.loadSkin", json!({ "path": path_str }))?;
        serde_json::from_value(resp).map_err(|e| e.to_string())
    }

    pub fn unload_skin(&self, id: &str) -> Result<(), String> {
        self.call("pluvia.unloadSkin", json!({ "id": id }))?;
        Ok(())
    }

    pub fn refresh_skin(&self, id: &str) -> Result<(), String> {
        self.call("pluvia.refreshSkin", json!({ "id": id }))?;
        Ok(())
    }

    pub fn refresh_all(&self) -> Result<usize, String> {
        let resp = self.call("pluvia.refreshAll", json!({}))?;
        let count = resp.get("refreshed_count").and_then(|v| v.as_u64()).unwrap_or(0);
        Ok(count as usize)
    }

    pub fn set_variable(&self, id: &str, key: &str, value: &str) -> Result<(), String> {
        self.call("pluvia.setVariable", json!({
            "id": id,
            "key": key,
            "value": value
        }))?;
        Ok(())
    }

    pub fn set_position(&self, id: &str, x: i32, y: i32) -> Result<(), String> {
        self.call("pluvia.setPosition", json!({
            "id": id,
            "x": x,
            "y": y
        }))?;
        Ok(())
    }

    pub fn set_opacity(&self, id: &str, opacity: f64) -> Result<(), String> {
        self.call("pluvia.setOpacity", json!({
            "id": id,
            "opacity": opacity
        }))?;
        Ok(())
    }

    pub fn import_package<P: AsRef<Path>>(&self, path: P) -> Result<Value, String> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        self.call("pluvia.importPackage", json!({ "path": path_str }))
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|e| format!("Cannot connect to Pluvia daemon: {}", e))?;

        let req = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });

        let mut req_str = req.to_string();
        req_str.push('\n');

        stream
            .write_all(req_str.as_bytes())
            .map_err(|e| format!("Failed to send command to daemon: {}", e))?;
        stream.flush().map_err(|e| e.to_string())?;

        let mut reader = BufReader::new(stream);
        let mut resp_line = String::new();
        reader
            .read_line(&mut resp_line)
            .map_err(|e| format!("Failed to read response from daemon: {}", e))?;

        let resp: Value = serde_json::from_str(resp_line.trim())
            .map_err(|e| format!("Failed to parse daemon response: {}", e))?;

        if let Some(err) = resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown RPC error");
            return Err(msg.to_string());
        }

        resp.get("result")
            .cloned()
            .ok_or_else(|| "Missing 'result' in response".to_string())
    }
}

pub fn default_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("PLUVIA_SOCKET") {
        return PathBuf::from(path);
    }
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("pluvia.sock");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata("/proc/self") {
            let uid = meta.uid();
            let run_user = PathBuf::from(format!("/run/user/{}", uid));
            if run_user.exists() {
                return run_user.join("pluvia.sock");
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache/pluvia/pluvia.sock");
    }
    PathBuf::from("/tmp/pluvia.sock")
}

pub fn find_daemon_binary() -> PathBuf {
    // 1. Same directory as current running executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("pluvia-daemon");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // 2. ~/.local/bin/pluvia-daemon
    if let Ok(home) = std::env::var("HOME") {
        let candidate = PathBuf::from(home).join(".local/bin/pluvia-daemon");
        if candidate.exists() {
            return candidate;
        }
    }
    // 3. System-wide in PATH
    PathBuf::from("pluvia-daemon")
}

