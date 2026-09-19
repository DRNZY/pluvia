use crate::runtime::SkinRuntime;
use pluvia_core::extractor::extract_rmskin_package;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, watch, RwLock};

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

pub struct IpcHandle {
    socket_path: PathBuf,
    shutdown_tx: watch::Sender<bool>,
    _task_handle: tokio::task::JoinHandle<()>,
}

impl IpcHandle {
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl Drop for IpcHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct IpcServer {
    socket_path: PathBuf,
    runtime: Arc<RwLock<SkinRuntime>>,
}

impl IpcServer {
    pub fn new<P: AsRef<Path>>(socket_path: P, runtime: Arc<RwLock<SkinRuntime>>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            runtime,
        }
    }

    pub fn spawn(self) -> Result<IpcHandle, std::io::Error> {
        if let Some(parent) = self.socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&self.socket_path);

        let listener = UnixListener::bind(&self.socket_path)?;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let (broadcast_tx, _) = broadcast::channel::<String>(256);

        let socket_path_clone = self.socket_path.clone();
        let runtime = self.runtime.clone();

        let task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, _addr)) => {
                                let rt = runtime.clone();
                                let b_tx = broadcast_tx.clone();
                                let b_rx = broadcast_tx.subscribe();
                                tokio::spawn(handle_client(stream, rt, b_tx, b_rx));
                            }
                            Err(e) => {
                                eprintln!("Unix socket accept error: {}", e);
                            }
                        }
                    }
                }
            }
        });

        Ok(IpcHandle {
            socket_path: socket_path_clone,
            shutdown_tx,
            _task_handle: task_handle,
        })
    }
}

async fn handle_client(
    stream: UnixStream,
    runtime: Arc<RwLock<SkinRuntime>>,
    broadcast_tx: broadcast::Sender<String>,
    mut broadcast_rx: broadcast::Receiver<String>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        tokio::select! {
            notif_res = broadcast_rx.recv() => {
                if let Ok(notif_str) = notif_res {
                    let mut payload = notif_str;
                    payload.push('\n');
                    if writer.write_all(payload.as_bytes()).await.is_err() {
                        break;
                    }
                    let _ = writer.flush().await;
                }
            }
            read_res = reader.read_line(&mut line) => {
                match read_res {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }

                        // Parse JSON-RPC
                        let resp = process_json_rpc(trimmed, &runtime, &broadcast_tx).await;
                        if let Some(resp_val) = resp {
                            let mut resp_str = resp_val.to_string();
                            resp_str.push('\n');
                            if writer.write_all(resp_str.as_bytes()).await.is_err() {
                                break;
                            }
                            let _ = writer.flush().await;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

async fn process_json_rpc(
    input: &str,
    runtime: &Arc<RwLock<SkinRuntime>>,
    broadcast_tx: &broadcast::Sender<String>,
) -> Option<Value> {
    let req: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(_) => {
            return Some(json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32700,
                    "message": "Parse error"
                },
                "id": Value::Null
            }));
        }
    };

    if !req.is_object() {
        return Some(json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32600,
                "message": "Invalid Request"
            },
            "id": Value::Null
        }));
    }

    let req_obj = req.as_object().unwrap();
    let id = req_obj.get("id").cloned();
    let is_notification = id.is_none();

    let jsonrpc = req_obj.get("jsonrpc").and_then(|v| v.as_str());
    if jsonrpc != Some("2.0") {
        if is_notification {
            return None;
        }
        return Some(json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32600,
                "message": "Invalid Request: 'jsonrpc' must be '2.0'"
            },
            "id": id.unwrap_or(Value::Null)
        }));
    }

    let method = match req_obj.get("method").and_then(|v| v.as_str()) {
        Some(m) => m,
        None => {
            if is_notification {
                return None;
            }
            return Some(json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32600,
                    "message": "Invalid Request: missing method"
                },
                "id": id.unwrap_or(Value::Null)
            }));
        }
    };

    let params = req_obj.get("params").cloned().unwrap_or(json!({}));

    // Method dispatch
    let result = execute_method(method, params, runtime, broadcast_tx).await;

    if is_notification {
        return None;
    }

    let id_val = id.unwrap_or(Value::Null);
    match result {
        Ok(res_val) => Some(json!({
            "jsonrpc": "2.0",
            "result": res_val,
            "id": id_val
        })),
        Err(RpcError { code, message, data }) => Some(json!({
            "jsonrpc": "2.0",
            "error": {
                "code": code,
                "message": message,
                "data": data
            },
            "id": id_val
        })),
    }
}

struct RpcError {
    code: i64,
    message: String,
    data: Option<Value>,
}

impl RpcError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method '{}' not found", method),
            data: None,
        }
    }

    fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: msg.to_string(),
            data: None,
        }
    }

    fn server_error(msg: String) -> Self {
        Self {
            code: -32000,
            message: msg,
            data: None,
        }
    }
}

async fn execute_method(
    method: &str,
    params: Value,
    runtime: &Arc<RwLock<SkinRuntime>>,
    broadcast_tx: &broadcast::Sender<String>,
) -> Result<Value, RpcError> {
    match method {
        "pluvia.ping" => Ok(json!("pong")),

        "pluvia.getState" => {
            let rt = runtime.read().await;
            let state = rt.get_state();
            Ok(json!(state))
        }

        "pluvia.listSkins" => {
            let rt = runtime.read().await;
            let skins = rt.list_skins();
            Ok(json!(skins))
        }

        "pluvia.loadSkin" => {
            let path = params
                .get("path")
                .and_then(|p| p.as_str())
                .or_else(|| params.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'path' is required"))?;

            let mut rt = runtime.write().await;
            let info = rt
                .load_skin(path)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            let notif = json!({
                "jsonrpc": "2.0",
                "method": "notify.skinLoaded",
                "params": {
                    "id": info.id,
                    "path": info.path
                }
            });
            let _ = broadcast_tx.send(notif.to_string());

            Ok(json!(info))
        }

        "pluvia.unloadSkin" => {
            let id = params
                .get("id")
                .and_then(|p| p.as_str())
                .or_else(|| params.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'id' is required"))?;

            let mut rt = runtime.write().await;
            rt.unload_skin(id)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            let notif = json!({
                "jsonrpc": "2.0",
                "method": "notify.skinUnloaded",
                "params": {
                    "id": id
                }
            });
            let _ = broadcast_tx.send(notif.to_string());

            Ok(json!({ "unloaded": true, "id": id }))
        }

        "pluvia.refreshSkin" => {
            let id = params
                .get("id")
                .and_then(|p| p.as_str())
                .or_else(|| params.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'id' is required"))?;

            let mut rt = runtime.write().await;
            rt.refresh_skin(id)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            Ok(json!({ "refreshed": true, "id": id }))
        }

        "pluvia.refreshAll" => {
            let mut rt = runtime.write().await;
            let count = rt
                .refresh_all()
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            Ok(json!({ "refreshed_count": count }))
        }

        "pluvia.setVariable" => {
            let id = params
                .get("id")
                .and_then(|p| p.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'id' is required"))?;
            let key = params
                .get("key")
                .and_then(|p| p.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'key' is required"))?;
            let value = params
                .get("value")
                .and_then(|p| p.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'value' is required"))?;

            let mut rt = runtime.write().await;
            rt.set_variable(id, key, value)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            Ok(json!({
                "success": true,
                "id": id,
                "key": key,
                "value": value
            }))
        }

        "pluvia.setPosition" => {
            let id = params
                .get("id")
                .and_then(|p| p.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'id' is required"))?;
            let x = params
                .get("x")
                .and_then(|p| p.as_i64())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'x' is required"))? as i32;
            let y = params
                .get("y")
                .and_then(|p| p.as_i64())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'y' is required"))? as i32;

            let mut rt = runtime.write().await;
            rt.set_position(id, x, y)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            Ok(json!({
                "success": true,
                "id": id,
                "x": x,
                "y": y
            }))
        }

        "pluvia.setOpacity" => {
            let id = params
                .get("id")
                .and_then(|p| p.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'id' is required"))?;
            let opacity = params
                .get("opacity")
                .and_then(|p| p.as_f64())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'opacity' is required"))?;

            let mut rt = runtime.write().await;
            rt.set_opacity(id, opacity)
                .map_err(|e| RpcError::server_error(e.to_string()))?;

            Ok(json!({
                "success": true,
                "id": id,
                "opacity": opacity
            }))
        }

        "pluvia.importPackage" => {
            let archive_path = params
                .get("archive_path")
                .and_then(|p| p.as_str())
                .or_else(|| params.as_str())
                .ok_or_else(|| RpcError::invalid_params("Parameter 'archive_path' is required"))?;

            let dest_root = if let Some(dest) = params.get("dest_root").and_then(|d| d.as_str()) {
                PathBuf::from(dest)
            } else if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(".local/share/pluvia/skins")
            } else {
                PathBuf::from("skins")
            };

            let report = extract_rmskin_package(archive_path, &dest_root)
                .map_err(|e| RpcError::server_error(format!("Extraction error: {e}")))?;

            let notif = json!({
                "jsonrpc": "2.0",
                "method": "notify.packageExtractProgress",
                "params": {
                    "archive_path": archive_path,
                    "dest_root": dest_root.to_string_lossy(),
                    "files_extracted": report.files_extracted,
                    "total_bytes": report.total_bytes
                }
            });
            let _ = broadcast_tx.send(notif.to_string());

            Ok(json!({
                "files_extracted": report.files_extracted,
                "total_bytes": report.total_bytes,
                "destination": dest_root.to_string_lossy()
            }))
        }

        _ => Err(RpcError::method_not_found(method)),
    }
}
