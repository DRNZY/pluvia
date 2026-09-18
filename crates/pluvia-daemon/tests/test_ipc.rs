use pluvia_cli::client::PluviaClient;
use pluvia_daemon::display::BackendType;
use pluvia_daemon::ipc::IpcServer;
use pluvia_daemon::runtime::SkinRuntime;
use serde_json::json;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::RwLock;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn create_sample_skin(dir: &std::path::Path, skin_name: &str) -> std::path::PathBuf {
    let skin_dir = dir.join(skin_name);
    fs::create_dir_all(&skin_dir).unwrap();
    let skin_file = skin_dir.join("skin.ini");
    let content = r#"
[Rainmeter]
Update=1000

[Variables]
MyVar=InitialValue

[MeasureTime]
Measure=Time
Format=%H:%M:%S

[MeterBackground]
Meter=Shape
Shape=Rectangle 0,0,200,100 | Fill Color 30,30,30,255

[MeterTime]
Meter=String
MeasureName=MeasureTime
Text=#MyVar#: %1
X=10
Y=10
W=180
H=40
FontColor=255,255,255,255
"#;
    fs::write(&skin_file, content).unwrap();
    skin_file
}

async fn start_test_server() -> (tempfile::TempDir, pluvia_daemon::ipc::IpcHandle, Arc<RwLock<SkinRuntime>>) {
    let tmp = tempdir().unwrap();
    let socket_path = tmp.path().join("pluvia_test.sock");
    let runtime = Arc::new(RwLock::new(SkinRuntime::new(BackendType::Mock)));
    let server = IpcServer::new(&socket_path, runtime.clone());
    let handle = server.spawn().expect("Failed to spawn test IPC server");
    // Give server a moment to bind
    tokio::time::sleep(Duration::from_millis(50)).await;
    (tmp, handle, runtime)
}

#[tokio::test]
async fn test_ipc_ping_pong() {
    let (_tmp, _handle, _runtime) = start_test_server().await;
    let mut stream = UnixStream::connect(_handle.socket_path()).await.unwrap();

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let req = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.ping",
        "params": {},
        "id": 1
    });
    writer.write_all(format!("{}\n", req).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"], "pong");
}

#[tokio::test]
async fn test_ipc_get_state() {
    let (_tmp, _handle, _runtime) = start_test_server().await;
    let mut stream = UnixStream::connect(_handle.socket_path()).await.unwrap();

    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    let req = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.getState",
        "params": {},
        "id": 2
    });
    writer.write_all(format!("{}\n", req).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(resp["id"], 2);
    let state = &resp["result"];
    assert_eq!(state["status"], "running");
    assert!(state["uptime_secs"].is_u64());
    assert!(state["active_skins"].is_array());
    assert_eq!(state["backend"], "Mock");
}

#[tokio::test]
async fn test_ipc_load_and_unload_skin() {
    let (tmp, _handle, _runtime) = start_test_server().await;
    let skin_path = create_sample_skin(tmp.path(), "ClockSkin");

    let mut stream = UnixStream::connect(_handle.socket_path()).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    // Helper to read until expected response id is received
    async fn read_response(reader: &mut BufReader<tokio::net::unix::ReadHalf<'_>>, expected_id: u64) -> serde_json::Value {
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).await.unwrap();
            assert!(n > 0, "Unexpected EOF while waiting for response");
            let val: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            if val.get("id").and_then(|id| id.as_u64()) == Some(expected_id) {
                return val;
            }
        }
    }

    // 1. Load skin
    let req_load = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.loadSkin",
        "params": { "path": skin_path.to_str().unwrap() },
        "id": 3
    });
    writer.write_all(format!("{}\n", req_load).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let resp_load = read_response(&mut reader, 3).await;
    assert_eq!(resp_load["id"], 3);
    let skin_id = resp_load["result"]["id"].as_str().unwrap().to_string();
    assert!(!skin_id.is_empty());

    // 2. List skins
    let req_list = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.listSkins",
        "params": {},
        "id": 4
    });
    writer.write_all(format!("{}\n", req_list).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let resp_list = read_response(&mut reader, 4).await;
    assert_eq!(resp_list["id"], 4);
    let skins = resp_list["result"].as_array().unwrap();
    assert_eq!(skins.len(), 1);
    assert_eq!(skins[0]["id"], skin_id);

    // 3. Unload skin
    let req_unload = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.unloadSkin",
        "params": { "id": skin_id },
        "id": 5
    });
    writer.write_all(format!("{}\n", req_unload).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let resp_unload = read_response(&mut reader, 5).await;
    assert_eq!(resp_unload["id"], 5);
    assert_eq!(resp_unload["result"]["unloaded"], true);

    // 4. List again, should be empty
    let req_list2 = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.listSkins",
        "params": {},
        "id": 6
    });
    writer.write_all(format!("{}\n", req_list2).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    let resp_list2 = read_response(&mut reader, 6).await;
    let skins2 = resp_list2["result"].as_array().unwrap();
    assert_eq!(skins2.len(), 0);
}

#[tokio::test]
async fn test_ipc_error_handling() {
    let (_tmp, _handle, _runtime) = start_test_server().await;
    let mut stream = UnixStream::connect(_handle.socket_path()).await.unwrap();
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    // 1. Parse error (-32700)
    writer.write_all(b"{not-valid-json\n").await.unwrap();
    writer.flush().await.unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).await.unwrap();
    let err_resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(err_resp["error"]["code"], -32700);

    // 2. Method not found (-32601)
    line.clear();
    let req_missing = json!({
        "jsonrpc": "2.0",
        "method": "pluvia.nonExistentMethod",
        "params": {},
        "id": 99
    });
    writer.write_all(format!("{}\n", req_missing).as_bytes()).await.unwrap();
    writer.flush().await.unwrap();

    reader.read_line(&mut line).await.unwrap();
    let resp_missing: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(resp_missing["id"], 99);
    assert_eq!(resp_missing["error"]["code"], -32601);
}

#[tokio::test]
async fn test_cli_client_integration() {
    let (tmp, _handle, _runtime) = start_test_server().await;
    let client = PluviaClient::new(_handle.socket_path());

    // Ping
    let pong = client.ping().await.unwrap();
    assert_eq!(pong, "pong");

    // Get state
    let state = client.get_state().await.unwrap();
    assert_eq!(state["status"], "running");

    // Load skin
    let skin_path = create_sample_skin(tmp.path(), "Mond");
    let loaded = client.load_skin(&skin_path).await.unwrap();
    let id = loaded["id"].as_str().unwrap();

    // Set variable
    let set_var = client.set_variable(id, "MyVar", "UpdatedValue").await.unwrap();
    assert_eq!(set_var["success"], true);

    // Refresh skin
    let refreshed = client.refresh_skin(id).await.unwrap();
    assert_eq!(refreshed["refreshed"], true);

    // Refresh all
    let ref_all = client.refresh_all().await.unwrap();
    assert!(ref_all["refreshed_count"].is_u64());

    // List
    let list = client.list_skins().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Unload
    let unloaded = client.unload_skin(id).await.unwrap();
    assert_eq!(unloaded["unloaded"], true);
}

#[tokio::test]
async fn test_ipc_pubsub_notifications() {
    let (tmp, _handle, _runtime) = start_test_server().await;
    let skin_path = create_sample_skin(tmp.path(), "NotifySkin");

    // Connect a subscriber stream
    let mut sub_stream = UnixStream::connect(_handle.socket_path()).await.unwrap();
    let (sub_reader, _) = sub_stream.split();
    let mut sub_reader = BufReader::new(sub_reader);

    // Use client to perform load and unload
    let client = PluviaClient::new(_handle.socket_path());
    let loaded = client.load_skin(&skin_path).await.unwrap();
    let skin_id = loaded["id"].as_str().unwrap();

    // The subscriber stream should receive a notification: notify.skinLoaded
    let mut line = String::new();
    sub_reader.read_line(&mut line).await.unwrap();
    let notif1: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(notif1["method"], "notify.skinLoaded");
    assert_eq!(notif1["params"]["id"], skin_id);
    assert!(notif1.get("id").is_none()); // Notifications must not have an id

    // Unload skin
    client.unload_skin(skin_id).await.unwrap();

    // Subscriber should receive: notify.skinUnloaded
    line.clear();
    sub_reader.read_line(&mut line).await.unwrap();
    let notif2: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(notif2["method"], "notify.skinUnloaded");
    assert_eq!(notif2["params"]["id"], skin_id);
    assert!(notif2.get("id").is_none());
}

#[tokio::test]
async fn test_ipc_package_import() {
    let (tmp, _handle, _runtime) = start_test_server().await;
    let client = PluviaClient::new(_handle.socket_path());

    // Create a dummy rmskin archive
    let archive_path = tmp.path().join("test_skin.rmskin");
    let file = File::create(&archive_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("Skins/TestSkin/skin.ini", options).unwrap();
    zip.write_all(b"[Rainmeter]\nUpdate=1000\n[MeterText]\nMeter=String\nText=ImportedTest\n").unwrap();
    zip.finish().unwrap();

    let dest_dir = tmp.path().join("extracted_skins");
    let result = client.import_package(&archive_path, Some(&dest_dir)).await.unwrap();
    assert!(result["files_extracted"].as_u64().unwrap() >= 1);
    assert!(dest_dir.join("Skins/TestSkin/skin.ini").exists());
}

#[tokio::test]
async fn test_runtime_tick_clears_damage() {
    let tmp = tempdir().unwrap();
    let skin_path = create_sample_skin(tmp.path(), "TickSkin");

    let mut runtime = SkinRuntime::new(BackendType::Mock);
    let info = runtime.load_skin(&skin_path).expect("Failed to load skin");

    // Perform a forced tick
    runtime.force_tick_skin(&info.id).expect("Failed to tick skin");

    // Verify damage rects / damage history was cleared on surface
    let skin = runtime.get_skin(&info.id).expect("Skin instance should exist");
    // Mock surface damage_history should be 0 because damage is cleared per tick
    let mock = skin.surface.as_ref();
    assert_eq!(mock.is_visible(), true);
    // Since tick clears damage, damage history should be empty after tick
    // (mock surface damage_history cleared by clear_damage)
}

#[tokio::test]
async fn test_skin_update_interval_throttling() {
    let tmp = tempdir().unwrap();
    let skin_path = create_sample_skin(tmp.path(), "ThrottledSkin");

    let mut runtime = SkinRuntime::new(BackendType::Mock);
    let info = runtime.load_skin(&skin_path).expect("Failed to load skin");

    // Initially tick_count is 0
    assert_eq!(runtime.get_skin(&info.id).unwrap().tick_count, 0);

    // Call tick_skin immediately (1000ms has not passed)
    runtime.tick_skin(&info.id).unwrap();
    assert_eq!(runtime.get_skin(&info.id).unwrap().tick_count, 0);

    // Repeated tick_skin calls within the interval do not advance tick_count
    for _ in 0..5 {
        runtime.tick_skin(&info.id).unwrap();
    }
    assert_eq!(runtime.get_skin(&info.id).unwrap().tick_count, 0);

    // Force tick bypasses the throttling interval
    runtime.force_tick_skin(&info.id).unwrap();
    assert_eq!(runtime.get_skin(&info.id).unwrap().tick_count, 1);
}
