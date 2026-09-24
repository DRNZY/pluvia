use pluvia_core::ini::MeterConfig;
use pluvia_core::render::meter_renderer::format_meter_value;

#[test]
fn test_autoscale_1024_bytes_and_megabytes() {
    let mut meter = MeterConfig::default();
    meter.name = "MeterRate".to_string();
    meter.properties.insert("autoscale".to_string(), "1".to_string());
    meter.properties.insert("numofdecimals".to_string(), "1".to_string());

    // 2967664 bytes -> 2.8 M
    let formatted = format_meter_value(2967664.0, &meter);
    assert_eq!(formatted, "2.8 M");

    // 500 bytes -> 500 B
    let formatted_bytes = format_meter_value(500.0, &meter);
    assert_eq!(formatted_bytes, "500 B");

    // 2048 bytes -> 2.0 k
    let formatted_k = format_meter_value(2048.0, &meter);
    assert_eq!(formatted_k, "2.0 k");
}

#[test]
fn test_num_of_decimals_and_scale() {
    let mut meter = MeterConfig::default();
    meter.name = "MeterCpu".to_string();
    meter.properties.insert("scale".to_string(), "10.0".to_string());
    meter.properties.insert("numofdecimals".to_string(), "2".to_string());

    // 283.0188 / 10.0 = 28.30188 -> 28.30
    let formatted = format_meter_value(283.0188, &meter);
    assert_eq!(formatted, "28.30");
}

#[test]
fn test_prefix_and_postfix_formatting() {
    let mut meter = MeterConfig::default();
    meter.name = "MeterDisk".to_string();
    meter.properties.insert("prefix".to_string(), "E: ".to_string());
    meter.properties.insert("postfix".to_string(), "B/s".to_string());
    meter.properties.insert("autoscale".to_string(), "1".to_string());
    meter.properties.insert("numofdecimals".to_string(), "1".to_string());

    let formatted = format_meter_value(2967664.0, &meter);
    assert_eq!(formatted, "E: 2.8 MB/s");
}

#[test]
fn test_percentual_formatting() {
    let mut meter = MeterConfig::default();
    meter.name = "MeterRam".to_string();
    meter.properties.insert("percentual".to_string(), "1".to_string());
    meter.properties.insert("numofdecimals".to_string(), "0".to_string());
    meter.properties.insert("postfix".to_string(), "%".to_string());

    // 0.65 -> 65%
    let formatted = format_meter_value(0.65, &meter);
    assert_eq!(formatted, "65%");
}
