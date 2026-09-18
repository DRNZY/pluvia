use chrono::TimeZone;
use pluvia_core::ini::MeasureConfig;
use pluvia_core::measures::mpris::{NowPlayingData, NowPlayingMeasure, PlayerType};
use pluvia_core::measures::system::{
    CpuMeasure, DiskMeasure, MemoryMeasure, NetMeasure, NetMeasureType, UptimeMeasure,
};
use pluvia_core::measures::time::TimeMeasure;
use pluvia_core::measures::{create_measure, Measure, MeasureValue};
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_measure_value_conversions() {
    let str_val = MeasureValue::String("42.5".to_string());
    assert_eq!(str_val.to_string_val(), "42.5");
    assert_eq!(str_val.to_number_val(), 42.5);
    assert_eq!(str_val.as_str(), Some("42.5"));
    assert_eq!(str_val.as_f64(), None);

    let num_val = MeasureValue::Number(100.0);
    assert_eq!(num_val.to_string_val(), "100");
    assert_eq!(num_val.to_number_val(), 100.0);
    assert_eq!(num_val.as_str(), None);
    assert_eq!(num_val.as_f64(), Some(100.0));

    let float_val = MeasureValue::Number(3.1415);
    assert_eq!(float_val.to_string_val(), "3.1415");
    assert_eq!(float_val.to_number_val(), 3.1415);

    // Display implementation
    assert_eq!(format!("{}", str_val), "42.5");
    assert_eq!(format!("{}", num_val), "100");
}

#[test]
fn test_time_measure_formatting() {
    let mut m = TimeMeasure::new("%Y-%m-%d");
    let val = m.update();
    if let MeasureValue::String(s) = val {
        assert_eq!(s.len(), 10);
        assert!(s.contains('-'));
    } else {
        panic!("Expected string value");
    }

    let mut m_time = TimeMeasure::new("%H:%M");
    let val_time = m_time.update();
    if let MeasureValue::String(s) = val_time {
        assert_eq!(s.len(), 5);
        assert!(s.contains(':'));
    } else {
        panic!("Expected string value");
    }

    // Windows %#d format code (strip leading zeros)
    let mut m_win = TimeMeasure::new("%Y-%#m-%#d");
    let dt = chrono::Local.with_ymd_and_hms(2026, 9, 5, 8, 30, 0).unwrap();
    m_win.set_custom_time(Some(dt));
    let val_win = m_win.update();
    assert_eq!(val_win.to_string_val(), "2026-9-5");

    // UTC timezone support
    let mut m_utc = TimeMeasure::new("%H:%M");
    m_utc.set_time_zone(Some("utc".to_string()));
    m_utc.set_custom_time(Some(dt));
    let val_utc = m_utc.update();
    assert_eq!(val_utc.to_string_val(), dt.naive_utc().format("%H:%M").to_string());
}

#[test]
fn test_cpu_measure_delta_calculation() {
    // Test real system CPU read
    let mut cpu = CpuMeasure::new();
    let val1 = cpu.update();
    assert!(val1.to_number_val() >= 0.0 && val1.to_number_val() <= 100.0);

    // Test mocked /proc/stat delta calculation
    let mut file1 = NamedTempFile::new().unwrap();
    // Sample tick 1: total = 100, idle = 80 -> work = 20
    writeln!(file1, "cpu  20 0 0 80 0 0 0 0 0 0").unwrap();
    let mut mock_cpu = CpuMeasure::with_path(file1.path().to_path_buf(), None);
    let initial = mock_cpu.update();
    assert_eq!(initial.to_number_val(), 0.0); // No previous tick yet

    // Tick 2: delta total = 100, delta idle = 50 -> delta work = 50 -> 50%
    let mut file2 = NamedTempFile::new().unwrap();
    writeln!(file2, "cpu  70 0 0 130 0 0 0 0 0 0").unwrap();
    mock_cpu.set_stat_path(file2.path().to_path_buf());
    let second = mock_cpu.update();
    assert!((second.to_number_val() - 50.0).abs() < 1e-3);
}

#[test]
fn test_cpu_measure_per_core() {
    let mut file1 = NamedTempFile::new().unwrap();
    writeln!(file1, "cpu  100 0 0 100 0 0 0 0 0 0").unwrap();
    writeln!(file1, "cpu0 50 0 0 50 0 0 0 0 0 0").unwrap();
    writeln!(file1, "cpu1 10 0 0 90 0 0 0 0 0 0").unwrap();

    let mut core0 = CpuMeasure::with_path(file1.path().to_path_buf(), Some(1));
    assert_eq!(core0.update().to_number_val(), 0.0);

    let mut file2 = NamedTempFile::new().unwrap();
    writeln!(file2, "cpu  200 0 0 200 0 0 0 0 0 0").unwrap();
    writeln!(file2, "cpu0 80 0 0 70 0 0 0 0 0 0").unwrap(); // delta total = 50, delta idle = 20, delta work = 30 -> 60%
    writeln!(file2, "cpu1 20 0 0 180 0 0 0 0 0 0").unwrap();

    core0.set_stat_path(file2.path().to_path_buf());
    let val = core0.update();
    assert!((val.to_number_val() - 60.0).abs() < 1e-3);
}

#[test]
fn test_system_memory_measure() {
    let mut mem = MemoryMeasure::new("used_percent");
    let val = mem.update();
    if let MeasureValue::Number(pct) = val {
        assert!(pct >= 0.0 && pct <= 100.0);
    } else {
        panic!("Expected numeric percent");
    }

    let mut mem_total = MemoryMeasure::new("total");
    let total_val = mem_total.update();
    assert!(total_val.to_number_val() > 0.0);

    let mut mem_used = MemoryMeasure::new("used");
    let used_val = mem_used.update();
    assert!(used_val.to_number_val() > 0.0);
}

#[test]
fn test_mocked_memory_measure() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "MemTotal:       16000000 kB").unwrap();
    writeln!(file, "MemFree:         4000000 kB").unwrap();
    writeln!(file, "MemAvailable:    8000000 kB").unwrap();
    writeln!(file, "SwapTotal:       8000000 kB").unwrap();
    writeln!(file, "SwapFree:        6000000 kB").unwrap();

    let mut mem_pct = MemoryMeasure::with_path(file.path().to_path_buf(), "used_percent");
    // total = 16GB, available = 8GB -> used = 8GB -> 50%
    assert_eq!(mem_pct.update().to_number_val(), 50.0);

    let mut mem_free = MemoryMeasure::with_path(file.path().to_path_buf(), "free");
    assert_eq!(mem_free.update().to_number_val(), 8000000.0 * 1024.0);

    let mut swap_pct = MemoryMeasure::with_path(file.path().to_path_buf(), "swap_percent");
    // swap total = 8GB, swap free = 6GB -> swap used = 2GB -> 25%
    assert_eq!(swap_pct.update().to_number_val(), 25.0);
}

#[test]
fn test_disk_measure() {
    let mut disk_free = DiskMeasure::new("/", "free");
    let free_bytes = disk_free.update().to_number_val();
    assert!(free_bytes > 0.0);

    let mut disk_total = DiskMeasure::new("/", "total");
    let total_bytes = disk_total.update().to_number_val();
    assert!(total_bytes >= free_bytes);

    let mut disk_pct = DiskMeasure::new("/", "percent");
    let pct = disk_pct.update().to_number_val();
    assert!(pct >= 0.0 && pct <= 100.0);

    // Non-existent path returns 0.0 gracefully
    let mut invalid = DiskMeasure::new("/nonexistent_path_404", "free");
    assert_eq!(invalid.update().to_number_val(), 0.0);
}

#[test]
fn test_uptime_measure() {
    let mut uptime = UptimeMeasure::new();
    let val = uptime.update();
    assert!(val.to_number_val() > 0.0);

    // Mocked uptime with Rainmeter formatting
    let mut file = NamedTempFile::new().unwrap();
    // 90061.5 seconds = 1 day, 1 hour, 1 minute, 1 second
    writeln!(file, "90061.50 12345.67").unwrap();
    let mut formatted_uptime = UptimeMeasure::with_path(file.path().to_path_buf())
        .with_format("%4!02d!:%3!02d!:%2!02d!:%1!02d!");
    let val = formatted_uptime.update();
    assert_eq!(val.to_string_val(), "01:01:01:01");
}

#[test]
fn test_net_measure() {
    let mut net_in = NetMeasure::new(NetMeasureType::In);
    let val = net_in.update();
    assert!(val.to_number_val() >= 0.0);

    let mut net_out = NetMeasure::new(NetMeasureType::Out);
    let val_out = net_out.update();
    assert!(val_out.to_number_val() >= 0.0);
}

#[test]
fn test_net_measure_filters_loopback_on_all_and_zero() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "Inter-|   Receive                                                |  Transmit").unwrap();
    writeln!(file, " face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed").unwrap();
    writeln!(file, "    lo: 1000000     100    0    0    0     0          0         0  1000000     100    0    0    0     0       0          0").unwrap();
    writeln!(file, "  eth0:     500      10    0    0    0     0          0         0      200      10    0    0    0     0       0          0").unwrap();

    // Interface="0"
    let mut net_zero = NetMeasure::with_path(
        file.path().to_path_buf(),
        NetMeasureType::In,
        Some("0".to_string()),
    );
    // Initial update caches base bytes
    net_zero.update();

    // Second tick with updated bytes
    let mut file2 = NamedTempFile::new().unwrap();
    writeln!(file2, "Inter-|   Receive                                                |  Transmit").unwrap();
    writeln!(file2, " face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo colls carrier compressed").unwrap();
    writeln!(file2, "    lo: 2000000     100    0    0    0     0          0         0  2000000     100    0    0    0     0       0          0").unwrap();
    writeln!(file2, "  eth0:     600      10    0    0    0     0          0         0      300      10    0    0    0     0       0          0").unwrap();

    std::thread::sleep(std::time::Duration::from_millis(50));
    net_zero.set_dev_path(file2.path().to_path_buf());
    let rate_zero = net_zero.update().to_number_val();
    // lo changed by 1,000,000, eth0 changed by 100.
    // Over 50ms (0.05s), 100 bytes / 0.05s ≈ 2000 B/s.
    // If lo were not excluded: 1,000,100 / 0.05s ≈ 20,000,000 B/s.
    assert!(rate_zero < 50_000.0, "Rate was {}, lo was not filtered!", rate_zero);

    // Interface="all"
    let mut net_all = NetMeasure::with_path(
        file.path().to_path_buf(),
        NetMeasureType::In,
        Some("all".to_string()),
    );
    net_all.update();
    std::thread::sleep(std::time::Duration::from_millis(50));
    net_all.set_dev_path(file2.path().to_path_buf());
    let rate_all = net_all.update().to_number_val();
    assert!(rate_all < 50_000.0, "Rate was {}, lo was not filtered on 'all'!", rate_all);
}


#[test]
fn test_mpris_now_playing_measure_real_and_mock() {
    // Real system query (falls back gracefully if no player running)
    let mut np = NowPlayingMeasure::new(PlayerType::Title);
    let val = np.update();
    assert!(matches!(val, MeasureValue::String(_)));

    // Mocked player data testing all PlayerTypes
    let mock = NowPlayingData {
        title: "Starboy".to_string(),
        artist: "The Weeknd".to_string(),
        album: "Starboy Album".to_string(),
        cover: "file:///path/to/cover.jpg".to_string(),
        state: 1, // Playing
        status: 1,
        duration: 230.0,
        position: 115.0,
    };

    let mut m_title = NowPlayingMeasure::new(PlayerType::Title).with_mock_data(mock.clone());
    assert_eq!(m_title.update().to_string_val(), "Starboy");

    let mut m_artist = NowPlayingMeasure::new(PlayerType::Artist).with_mock_data(mock.clone());
    assert_eq!(m_artist.update().to_string_val(), "The Weeknd");

    let mut m_album = NowPlayingMeasure::new(PlayerType::Album).with_mock_data(mock.clone());
    assert_eq!(m_album.update().to_string_val(), "Starboy Album");

    let mut m_cover = NowPlayingMeasure::new(PlayerType::Cover).with_mock_data(mock.clone());
    assert_eq!(m_cover.update().to_string_val(), "file:///path/to/cover.jpg");

    let mut m_state = NowPlayingMeasure::new(PlayerType::State).with_mock_data(mock.clone());
    assert_eq!(m_state.update().to_number_val(), 1.0);

    let mut m_prog = NowPlayingMeasure::new(PlayerType::Progress).with_mock_data(mock.clone());
    assert_eq!(m_prog.update().to_number_val(), 50.0);
}

#[test]
fn test_measure_factory_from_config() {
    // Time measure
    let mut props = HashMap::new();
    props.insert("format".to_string(), "%H:%M".to_string());
    let cfg = MeasureConfig {
        name: "MeasureTime".to_string(),
        measure_type: "Time".to_string(),
        plugin: None,
        format: Some("%H:%M".to_string()),
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: props,
    };
    let mut measure = create_measure(&cfg).expect("Failed to create Time measure");
    let val = measure.update();
    assert!(matches!(val, MeasureValue::String(_)));

    // CPU measure
    let cpu_props = HashMap::new();
    let cpu_cfg = MeasureConfig {
        name: "MeasureCPU".to_string(),
        measure_type: "CPU".to_string(),
        plugin: None,
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: cpu_props,
    };
    let mut cpu_measure = create_measure(&cpu_cfg).expect("Failed to create CPU measure");
    let val = cpu_measure.update();
    assert!(matches!(val, MeasureValue::Number(_)));

    // Memory measure
    let mut mem_props = HashMap::new();
    mem_props.insert("percent".to_string(), "1".to_string());
    let mem_cfg = MeasureConfig {
        name: "MeasureMem".to_string(),
        measure_type: "PhysicalMemory".to_string(),
        plugin: None,
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: mem_props,
    };
    let mut mem_measure = create_measure(&mem_cfg).expect("Failed to create Memory measure");
    assert!(mem_measure.update().to_number_val() >= 0.0);

    // FreeDiskSpace measure
    let mut disk_props = HashMap::new();
    disk_props.insert("drive".to_string(), "C:".to_string());
    let disk_cfg = MeasureConfig {
        name: "MeasureDisk".to_string(),
        measure_type: "FreeDiskSpace".to_string(),
        plugin: None,
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: disk_props,
    };
    let mut disk_measure = create_measure(&disk_cfg).expect("Failed to create Disk measure");
    assert!(disk_measure.update().to_number_val() > 0.0);

    // Uptime measure
    let uptime_cfg = MeasureConfig {
        name: "MeasureUptime".to_string(),
        measure_type: "Uptime".to_string(),
        plugin: None,
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: HashMap::new(),
    };
    let mut uptime_measure = create_measure(&uptime_cfg).expect("Failed to create Uptime measure");
    assert!(uptime_measure.update().to_number_val() > 0.0);

    // NetIn measure
    let net_cfg = MeasureConfig {
        name: "MeasureNet".to_string(),
        measure_type: "NetIn".to_string(),
        plugin: None,
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: HashMap::new(),
    };
    let mut net_measure = create_measure(&net_cfg).expect("Failed to create Net measure");
    assert!(net_measure.update().to_number_val() >= 0.0);

    // NowPlaying plugin measure
    let mut np_props = HashMap::new();
    np_props.insert("playertype".to_string(), "artist".to_string());
    let np_cfg = MeasureConfig {
        name: "MeasureNP".to_string(),
        measure_type: "Plugin".to_string(),
        plugin: Some("NowPlaying.dll".to_string()),
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: np_props,
    };
    let mut np_measure = create_measure(&np_cfg).expect("Failed to create NowPlaying measure");
    assert!(matches!(np_measure.update(), MeasureValue::String(_)));
}
