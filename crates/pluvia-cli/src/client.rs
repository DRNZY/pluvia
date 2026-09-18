use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

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

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("JSON-RPC error ({code}): {message}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<Value>,
    },
    #[error("Connection closed by daemon")]
    ConnectionClosed,
    #[error("Protocol error: {0}")]
    Protocol(String),
}

#[derive(Debug, Clone)]
pub struct PluviaClient {
    socket_path: PathBuf,
    id_counter: Arc<AtomicU64>,
}

impl PluviaClient {
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            id_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);

        let id = self.id_counter.fetch_add(1, Ordering::SeqCst);
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id
        });

        let mut payload = serde_json::to_string(&req)?;
        payload.push('\n');
        writer.write_all(payload.as_bytes()).await?;
        writer.flush().await?;

        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(ClientError::ConnectionClosed);
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let val: Value = serde_json::from_str(trimmed)?;
            if let Some(resp_id) = val.get("id") {
                if resp_id.as_u64() == Some(id) {
                    if let Some(err) = val.get("error").filter(|e| !e.is_null()) {
                        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-32000);
                        let message = err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("Unknown error")
                            .to_string();
                        let data = err.get("data").cloned();
                        return Err(ClientError::Rpc {
                            code,
                            message,
                            data,
                        });
                    }
                    if let Some(result) = val.get("result") {
                        return Ok(result.clone());
                    }
                    return Ok(Value::Null);
                }
            }
        }
    }

    pub async fn ping(&self) -> Result<String, ClientError> {
        let res = self.call("pluvia.ping", serde_json::json!({})).await?;
        res.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ClientError::Protocol("Expected string result for ping".into()))
    }

    pub async fn get_state(&self) -> Result<Value, ClientError> {
        self.call("pluvia.getState", serde_json::json!({})).await
    }

    pub async fn load_skin<P: AsRef<Path>>(&self, path: P) -> Result<Value, ClientError> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        self.call("pluvia.loadSkin", serde_json::json!({ "path": path_str }))
            .await
    }

    pub async fn unload_skin(&self, id: &str) -> Result<Value, ClientError> {
        self.call("pluvia.unloadSkin", serde_json::json!({ "id": id }))
            .await
    }

    pub async fn list_skins(&self) -> Result<Value, ClientError> {
        self.call("pluvia.listSkins", serde_json::json!({})).await
    }

    pub async fn refresh_skin(&self, id: &str) -> Result<Value, ClientError> {
        self.call("pluvia.refreshSkin", serde_json::json!({ "id": id }))
            .await
    }

    pub async fn refresh_all(&self) -> Result<Value, ClientError> {
        self.call("pluvia.refreshAll", serde_json::json!({})).await
    }

    pub async fn set_variable(
        &self,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<Value, ClientError> {
        self.call(
            "pluvia.setVariable",
            serde_json::json!({ "id": id, "key": key, "value": value }),
        )
        .await
    }

    pub async fn import_package<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        archive_path: P,
        dest_root: Option<Q>,
    ) -> Result<Value, ClientError> {
        let arch_str = archive_path.as_ref().to_string_lossy().to_string();
        let mut params = serde_json::json!({ "archive_path": arch_str });
        if let Some(dest) = dest_root {
            params["dest_root"] =
                Value::String(dest.as_ref().to_string_lossy().to_string());
        }
        self.call("pluvia.importPackage", params).await
    }
}
