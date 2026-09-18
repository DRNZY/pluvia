use pluvia_core::ini::MeasureConfig;
use pluvia_core::measures::{create_measure, Measure, MeasureValue};
use pluvia_core::plugins::action_timer::ActionTimerPlugin;
use pluvia_core::plugins::audio_level::{AudioChannel, AudioLevelPlugin, AudioLevelType};
use pluvia_core::plugins::process::ProcessPlugin;
use pluvia_core::plugins::web_parser::WebParserPlugin;
use pluvia_core::plugins::win7_audio::Win7AudioPlugin;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_action_timer_plugin_queuing_waits_repeats_and_variables() {
    let mut timer = ActionTimerPlugin::new();
    timer.set_variable("MyY", "0");
    timer.add_action("MoveDown", "[!SetVariable MyY (#MyY#+5)]");
    timer.add_action("Reset", "[!SetVariable MyY 0]");

    // ActionList: Reset -> Wait 10ms -> Repeat MoveDown 3 times (10ms interval)
    timer.add_action_list(1, "Reset | Wait 10 | Repeat MoveDown, 10, 3");

    assert!(!timer.is_running());
    timer.execute(1);
    assert!(timer.is_running());

    // Step 0: executes Reset immediately, starts Wait 10
    timer.step(0);
    assert_eq!(timer.get_variable("MyY"), Some("0"));

    // Step 10ms: Wait finishes, first repeat executes MoveDown (+5)
    timer.step(10);
    assert_eq!(timer.get_variable("MyY"), Some("5"));

    // Step 10ms: second repeat (+5)
    timer.step(10);
    assert_eq!(timer.get_variable("MyY"), Some("10"));

    // Step 10ms: third repeat (+5) and completes
    timer.step(10);
    assert_eq!(timer.get_variable("MyY"), Some("15"));
    assert!(!timer.is_running());

    // Test command interface: Execute and Stop
    timer.command("Execute 1");
    assert!(timer.is_running());
    timer.command("Stop 1");
    assert!(!timer.is_running());

    // Test Measure trait implementation
    let val = timer.update();
    assert_eq!(val.to_number_val(), 0.0);
}

#[test]
fn test_audio_level_plugin_fft_rms_peak_and_smoothing() {
    // 1. RMS and Peak with known constant amplitude
    let mut audio_rms = AudioLevelPlugin::new(AudioLevelType::RMS);
    let mut audio_peak = AudioLevelPlugin::new(AudioLevelType::Peak);

    // DC signal at 0.5 amplitude
    let samples = vec![0.5f32; 1024];
    audio_rms.feed_samples(&samples);
    audio_peak.feed_samples(&samples);

    let rms_val = audio_rms.update().to_number_val();
    let peak_val = audio_peak.update().to_number_val();

    assert!((rms_val - 0.5).abs() < 1e-2, "RMS expected ~0.5, got {}", rms_val);
    assert!((peak_val - 0.5).abs() < 1e-2, "Peak expected 0.5, got {}", peak_val);

    // Test stereo feeding
    let mut audio_stereo = AudioLevelPlugin::new(AudioLevelType::Peak)
        .with_channel(AudioChannel::Left);
    audio_stereo.feed_stereo(&[0.8; 512], &[0.2; 512]);
    assert!((audio_stereo.update().to_number_val() - 0.8).abs() < 1e-2);

    // 2. FFT Band calculation with a 1000 Hz pure tone
    let sample_rate = 44100.0f64;
    let freq = 1000.0f64;
    let mut sine_samples = Vec::with_capacity(1024);
    for i in 0..1024 {
        let t = i as f64 / sample_rate;
        sine_samples.push((2.0 * std::f64::consts::PI * freq * t).sin() as f32);
    }

    let mut audio_fft = AudioLevelPlugin::new(AudioLevelType::FFT)
        .with_bands(8)
        .with_freq_range(20.0, 20000.0)
        .with_band_idx(0);

    audio_fft.feed_samples(&sine_samples);
    let bands = audio_fft.get_bands();
    assert_eq!(bands.len(), 8);

    // Tone at 1000 Hz should appear in middle bands, not extreme edge (e.g., band 0 is 20-50Hz)
    let max_band_idx = bands
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    assert!(
        max_band_idx >= 1 && max_band_idx <= 6,
        "1000Hz peak was at band index {}",
        max_band_idx
    );

    // 3. Decay smoothing
    let mut audio_decay = AudioLevelPlugin::new(AudioLevelType::Peak)
        .with_smoothing(0.0, 200.0); // 0ms attack, 200ms decay
    audio_decay.feed_samples(&[1.0f32; 512]);
    let initial_val = audio_decay.update().to_number_val();
    assert!((initial_val - 1.0).abs() < 1e-2);

    // Now feed silence; smoothed value should decay gradually rather than dropping immediately to 0
    audio_decay.feed_samples(&[0.0f32; 512]);
    let decayed_val = audio_decay.update().to_number_val();
    assert!(decayed_val > 0.0 && decayed_val < 1.0, "Decayed val was {}", decayed_val);
}

#[test]
fn test_process_plugin_detects_processes() {
    let dir = tempdir().unwrap();
    let proc_path = dir.path().to_path_buf();

    // Mock active process 1001: notepad.exe
    let p1 = proc_path.join("1001");
    create_dir_all(&p1).unwrap();
    let mut f_comm = File::create(p1.join("comm")).unwrap();
    writeln!(f_comm, "notepad.exe").unwrap();

    // Mock active process 1002: discord
    let p2 = proc_path.join("1002");
    create_dir_all(&p2).unwrap();
    let mut f_comm2 = File::create(p2.join("comm")).unwrap();
    writeln!(f_comm2, "discord").unwrap();

    // Matching exact name with .exe
    let mut p_notepad = ProcessPlugin::new("notepad.exe").with_proc_dir(proc_path.clone());
    assert_eq!(p_notepad.update().to_number_val(), 1.0);
    assert!(p_notepad.is_running());

    // Matching without .exe when process has .exe
    let mut p_notepad_no_ext = ProcessPlugin::new("notepad").with_proc_dir(proc_path.clone());
    assert_eq!(p_notepad_no_ext.update().to_number_val(), 1.0);

    // Matching process without .exe
    let mut p_discord = ProcessPlugin::new("discord").with_proc_dir(proc_path.clone());
    assert_eq!(p_discord.update().to_number_val(), 1.0);

    // Nonexistent process
    let mut p_missing = ProcessPlugin::new("nonexistent_app").with_proc_dir(proc_path);
    assert_eq!(p_missing.update().to_number_val(), -1.0);
    assert!(!p_missing.is_running());
}

#[test]
fn test_web_parser_plugin_regex_extraction_and_mock_data() {
    let html_data = r#"
        <div id="weather">
            <span class="city">Stockholm</span>
            <span class="temp">18°C</span>
            <span class="humidity">65%</span>
        </div>
    "#;

    let pattern = r#"class="city">([^<]+)</span>\s*<span class="temp">([^<]+)</span>"#;

    // StringIndex 1 -> City
    let mut p_city = WebParserPlugin::new()
        .with_regex(pattern)
        .with_string_index(1)
        .with_mock_data(html_data);
    let val_city = p_city.update();
    assert_eq!(val_city.to_string_val(), "Stockholm");

    // StringIndex 2 -> Temp
    let mut p_temp = WebParserPlugin::new()
        .with_regex(pattern)
        .with_string_index(2)
        .with_mock_data(html_data);
    let val_temp = p_temp.update();
    assert_eq!(val_temp.to_string_val(), "18°C");

    // Test file download caching
    let dir = tempdir().unwrap();
    let target = dir.path().join("weather.html");
    let downloaded = p_city.download_to_file(&target).unwrap();
    assert!(downloaded.exists());
    let saved_content = std::fs::read_to_string(&downloaded).unwrap();
    assert!(saved_content.contains("Stockholm"));
}

#[test]
fn test_win7_audio_plugin_volume_calculations() {
    let mut audio = Win7AudioPlugin::new().with_mock(50.0, false, "Headphones");

    // Initial volume
    assert_eq!(audio.update().to_number_val(), 50.0);
    assert_eq!(audio.get_device_name(), "Headphones");

    // Command: SetVolume
    audio.command("SetVolume 75");
    assert_eq!(audio.get_volume(), 75.0);
    assert_eq!(audio.update().to_number_val(), 75.0);

    // Command: ChangeVolume positive and clamp to 100
    audio.command("ChangeVolume +40");
    assert_eq!(audio.get_volume(), 100.0);

    // Command: ChangeVolume negative and clamp to 0
    audio.command("ChangeVolume -150");
    assert_eq!(audio.get_volume(), 0.0);

    // Command: ToggleMute (returns -1.0 when muted)
    audio.command("SetVolume 60");
    audio.command("ToggleMute");
    assert!(audio.is_muted());
    assert_eq!(audio.update().to_number_val(), -1.0);

    // Toggle back
    audio.command("ToggleMute");
    assert!(!audio.is_muted());
    assert_eq!(audio.update().to_number_val(), 60.0);
}

#[test]
fn test_create_measure_factory_plugins() {
    // Helper to create MeasureConfig
    let make_cfg = |name: &str, mtype: &str, plugin: Option<&str>, props: Vec<(&str, &str)>| {
        let mut map = HashMap::new();
        for (k, v) in props {
            map.insert(k.to_ascii_lowercase(), v.to_string());
        }
        MeasureConfig {
            name: name.to_string(),
            measure_type: mtype.to_string(),
            plugin: plugin.map(|s| s.to_string()),
            format: None,
            formula: None,
            update_divider: 1,
            disabled: false,
            dynamic_variables: false,
            properties: map,
        }
    };

    // 1. ActionTimer
    let cfg1 = make_cfg("AT", "Plugin", Some("ActionTimer.dll"), vec![
        ("ActionList1", "Wait 10 | Move"),
        ("Move", "[!SetVariable X 1]"),
    ]);
    let mut m1 = create_measure(&cfg1).expect("Factory should create ActionTimer");
    assert!(matches!(m1.update(), MeasureValue::Number(_)));

    // 2. AudioLevel
    let cfg2 = make_cfg("AL", "Plugin", Some("AudioLevel"), vec![
        ("Port", "Output"),
        ("Type", "RMS"),
    ]);
    let mut m2 = create_measure(&cfg2).expect("Factory should create AudioLevel");
    assert!(matches!(m2.update(), MeasureValue::Number(_)));

    // 3. Win7Audio
    let cfg3 = make_cfg("W7", "Plugin", Some("Win7Audio"), vec![]);
    let mut m3 = create_measure(&cfg3).expect("Factory should create Win7Audio");
    assert!(matches!(m3.update(), MeasureValue::Number(_)));

    // 4. Process
    let cfg4 = make_cfg("Proc", "Plugin", Some("Process.dll"), vec![
        ("ProcessName", "systemd"),
    ]);
    let mut m4 = create_measure(&cfg4).expect("Factory should create Process");
    assert!(matches!(m4.update(), MeasureValue::Number(_)));

    // 5. WebParser
    let cfg5 = make_cfg("WP", "Plugin", Some("WebParser"), vec![
        ("URL", "https://example.com"),
        ("RegEx", "(.*)"),
    ]);
    let mut m5 = create_measure(&cfg5).expect("Factory should create WebParser");
    assert!(matches!(m5.update(), MeasureValue::String(_) | MeasureValue::Number(_)));

    // Also check direct Measure=ActionTimer without Measure=Plugin
    let cfg_direct = make_cfg("DirectAT", "ActionTimer", None, vec![]);
    assert!(create_measure(&cfg_direct).is_some());
}
