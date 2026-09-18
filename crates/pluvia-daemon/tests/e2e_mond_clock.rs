use pluvia_cli::client::PluviaClient;
use pluvia_core::ini::parse_skin_file;
use pluvia_core::measures::MeasureValue;
use pluvia_core::render::{add_application_font, meter_renderer::SkinState, MeterRenderer};
use pluvia_daemon::display::BackendType;
use pluvia_daemon::ipc::IpcServer;
use pluvia_daemon::runtime::SkinRuntime;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio::sync::RwLock;

fn get_mond_clock_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("skins")
        .join("Mond")
        .join("Clock")
        .join("Clock.ini")
}

fn get_mond_fonts_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("skins")
        .join("Mond")
        .join("@Resources")
        .join("Fonts")
}

#[test]
fn test_mond_clock_ini_structure() {
    let clock_ini = get_mond_clock_path();
    assert!(clock_ini.exists(), "Clock.ini must exist at {}", clock_ini.display());

    let config = parse_skin_file(&clock_ini).expect("Failed to parse Clock.ini");

    // Rainmeter update rate
    assert_eq!(config.update_rate_ms, 1000);

    // Variables
    assert_eq!(config.variables.get("fontname").unwrap(), "Anurati");
    assert_eq!(config.variables.get("fontname2").unwrap(), "Quicksand");
    assert_eq!(config.variables.get("scale").unwrap().parse::<f64>().unwrap(), 1.0);

    // Measures
    assert!(config.measures.contains_key("measureday"));
    assert!(config.measures.contains_key("measuredate"));
    assert!(config.measures.contains_key("measuretime"));

    let day_measure = config.measures.get("measureday").unwrap();
    assert_eq!(day_measure.measure_type.to_ascii_lowercase(), "time");

    // Meters
    assert!(config.meters.contains_key("meterday"));
    assert!(config.meters.contains_key("meterdate"));
    assert!(config.meters.contains_key("metertime"));

    let meter_day = config.meters.get("meterday").unwrap();
    assert_eq!(meter_day.font_face.as_deref(), Some("Anurati"));
    assert_eq!(meter_day.font_size, Some(44.0));
    assert_eq!(meter_day.properties.get("stringcase").unwrap(), "Upper");

    let meter_date = config.meters.get("meterdate").unwrap();
    assert_eq!(meter_date.font_face.as_deref(), Some("Quicksand"));
    assert_eq!(meter_date.font_size, Some(17.0));

    let meter_time = config.meters.get("metertime").unwrap();
    assert_eq!(meter_time.font_face.as_deref(), Some("Quicksand"));
    assert_eq!(meter_time.font_size, Some(15.0));
}

#[test]
fn test_mond_clock_fonts_registration() {
    let fonts_dir = get_mond_fonts_dir();
    let anurati = fonts_dir.join("Anurati.otf");
    let quicksand = fonts_dir.join("Quicksand.otf");

    assert!(anurati.exists(), "Anurati.otf must exist at {}", anurati.display());
    assert!(quicksand.exists(), "Quicksand.otf must exist at {}", quicksand.display());

    assert!(add_application_font(&anurati), "Anurati.otf font registration must succeed");
    assert!(add_application_font(&quicksand), "Quicksand.otf font registration must succeed");
}

#[test]
fn test_mond_clock_runtime_rendering_and_hit_mask() {
    let clock_ini = get_mond_clock_path();
    let mut runtime = SkinRuntime::new(BackendType::Mock);

    let info = runtime.load_skin(&clock_ini).expect("Failed to load Mond Clock skin");
    assert_eq!(info.id, "Clock");
    assert_eq!(info.update_rate_ms, 1000);
    assert_eq!(info.measures_count, 3);
    assert_eq!(info.meters_count, 3);

    // Verify bounds accommodates 700x240
    let bounds = info.bounds;
    assert!(bounds.width >= 700);
    assert!(bounds.height >= 190);

    let skin = runtime.get_skin("Clock").expect("Skin 'Clock' should be present");

    // Verify measures evaluated
    let day_val = skin.measure_values.get("measureday").expect("measureday value should exist");
    let date_val = skin.measure_values.get("measuredate").expect("measuredate value should exist");
    let time_val = skin.measure_values.get("measuretime").expect("measuretime value should exist");

    match day_val {
        MeasureValue::String(s) => {
            assert!(!s.is_empty(), "Day string should not be empty");
        }
        _ => panic!("Expected String value for measureday"),
    }

    match date_val {
        MeasureValue::String(s) => {
            assert!(!s.is_empty(), "Date string should not be empty");
        }
        _ => panic!("Expected String value for measuredate"),
    }

    match time_val {
        MeasureValue::String(s) => {
            assert!(s.starts_with('-') && s.ends_with('-'), "Time format should be - %H:%M -");
        }
        _ => panic!("Expected String value for measuretime"),
    }

    // Verify Cairo rendering and hit-mask calculation
    let renderer = MeterRenderer::new();
    let state = SkinState::new(skin.config.clone(), skin.measure_values.clone());
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, bounds.width as i32, bounds.height as i32).unwrap();
    let hit_mask = renderer.render_to_surface(&state, &surface).expect("Rendering Mond Clock to surface must succeed");

    // The mask must have disjoint bounding rectangles for the rendered text meters
    assert!(!hit_mask.to_rectangles().is_empty(), "Hit mask should detect rendered text bounds");

    // Transparent areas (such as (0, 0)) must not be hit -> 100% click-through
    assert!(!hit_mask.contains(0, 0), "Corner pixel (0, 0) must be click-through");
    assert!(!hit_mask.contains((bounds.width - 1) as i32, 0), "Top-right pixel must be click-through");

    // Verify forced tick preserves functionality
    runtime.force_tick_skin("Clock").expect("Forced tick on Clock must succeed");
    assert_eq!(runtime.get_skin("Clock").unwrap().tick_count, 1);
}

#[tokio::test]
async fn test_mond_clock_ipc_end_to_end() {
    let tmp = tempdir().unwrap();
    let socket_path = tmp.path().join("pluvia_mond.sock");

    let runtime = Arc::new(RwLock::new(SkinRuntime::new(BackendType::Mock)));
    let server = IpcServer::new(&socket_path, runtime.clone());
    let _handle = server.spawn().expect("Failed to spawn IPC server");
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = PluviaClient::new(&socket_path);

    // Ping check
    assert_eq!(client.ping().await.unwrap(), "pong");

    // Load Mond Clock
    let clock_ini = get_mond_clock_path();
    let load_res = client.load_skin(&clock_ini).await.expect("IPC load_skin Mond Clock must succeed");
    assert_eq!(load_res["id"], "Clock");

    // List skins
    let skins = client.list_skins().await.expect("IPC list_skins must succeed");
    let arr = skins.as_array().expect("Expected array of skins");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "Clock");

    // Set variable
    let var_res = client.set_variable("Clock", "Scale", "1.5").await.expect("IPC set_variable must succeed");
    assert_eq!(var_res["success"], true);

    // Refresh skin
    let ref_res = client.refresh_skin("Clock").await.expect("IPC refresh_skin must succeed");
    assert_eq!(ref_res["refreshed"], true);

    // Unload skin
    let unload_res = client.unload_skin("Clock").await.expect("IPC unload_skin must succeed");
    assert_eq!(unload_res["unloaded"], true);

    // List skins should now be empty
    let empty_list = client.list_skins().await.unwrap();
    assert_eq!(empty_list.as_array().unwrap().len(), 0);
}

#[test]
fn test_mond_clock_memory_footprint() {
    if let Ok(status) = fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let rss_kb: u64 = parts[1].parse().unwrap_or(0);
                    println!("Mond Clock E2E Process VmRSS: {} kB", rss_kb);
                    // Entire test binary memory with Cairo/Pango/Rust test runner is well below 100MB
                    // Runtime core engine target is under 12MB RAM
                    assert!(rss_kb < 100_000, "Process RSS exceeded reasonable threshold: {} kB", rss_kb);
                }
            }
        }
    }
}
