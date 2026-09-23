use pluvia_daemon::display::BackendType;
use pluvia_daemon::runtime::SkinRuntime;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_mouse_click_dispatches_meter_bangs() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "[Rainmeter]").unwrap();
    writeln!(file, "Update=1000").unwrap();
    writeln!(file, "").unwrap();
    writeln!(file, "[Variables]").unwrap();
    writeln!(file, "PlayState=Stopped").unwrap();
    writeln!(file, "").unwrap();
    writeln!(file, "[MeterPlayButton]").unwrap();
    writeln!(file, "Meter=String").unwrap();
    writeln!(file, "X=10").unwrap();
    writeln!(file, "Y=10").unwrap();
    writeln!(file, "W=80").unwrap();
    writeln!(file, "H=30").unwrap();
    writeln!(file, "Text=#PlayState#").unwrap();
    writeln!(file, "LeftMouseUpAction=[!SetVariable PlayState Playing]").unwrap();

    let mut runtime = SkinRuntime::new(BackendType::Mock);
    let skin_info = runtime.load_skin(file.path()).expect("Failed to load skin");

    let initial_skin = runtime.get_skin(&skin_info.id).unwrap();
    assert_eq!(initial_skin.config.variables.get("playstate"), Some("Stopped"));

    // Click at (x=20, y=20) inside MeterPlayButton (10..90, 10..40)
    let handled = runtime.handle_mouse_click(&skin_info.id, 20.0, 20.0, 1).expect("Click handling error");
    assert!(handled, "Mouse click should have been handled by MeterPlayButton");

    let updated_skin = runtime.get_skin(&skin_info.id).unwrap();
    assert_eq!(updated_skin.config.variables.get("playstate"), Some("Playing"));

    // Click outside meter at (x=200, y=200)
    let handled_outside = runtime.handle_mouse_click(&skin_info.id, 200.0, 200.0, 1).unwrap();
    assert!(!handled_outside, "Mouse click outside bounds should not trigger action");
}
