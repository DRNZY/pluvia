use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use chrono::{DateTime, Local, TimeZone};
use std::fmt::Write;

fn convert_rainmeter_time_format(raw_fmt: &str) -> String {
    let mut fmt = raw_fmt.trim().trim_matches('"').trim_matches('\'').to_string();
    if fmt.is_empty() {
        return "%H:%M:%S".to_string();
    }
    // Rainmeter Windows CRT strftime format conversions
    fmt = fmt.replace("%#x", "%A, %B %-d, %Y");
    fmt = fmt.replace("%#c", "%A, %B %-d, %Y %-H:%M:%S");
    fmt = fmt.replace("%#X", "%-H:%M:%S");
    fmt = fmt.replace("%#d", "%-d");
    fmt = fmt.replace("%#m", "%-m");
    fmt = fmt.replace("%#H", "%-H");
    fmt = fmt.replace("%#I", "%-I");
    fmt = fmt.replace("%#M", "%-M");
    fmt = fmt.replace("%#S", "%-S");
    fmt = fmt.replace("%#y", "%-y");
    fmt = fmt.replace("%#Y", "%Y");
    fmt = fmt.replace("%#j", "%-j");
    fmt = fmt.replace("%#U", "%-U");
    fmt = fmt.replace("%#W", "%-W");
    fmt = fmt.replace("%#w", "%-w");
    fmt = fmt.replace("%#z", "%z");
    fmt = fmt.replace("%#Z", "%Z");
    fmt
}

fn safe_format_time<Tz: TimeZone>(dt: &DateTime<Tz>, fmt_str: &str) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let mut buf = String::new();
    if write!(&mut buf, "{}", dt.format(fmt_str)).is_ok() {
        buf
    } else {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

/// Measure that produces local or formatted time strings via strftime.
#[derive(Debug, Clone)]
pub struct TimeMeasure {
    format: String,
    time_zone: Option<String>,
    current_value: MeasureValue,
    custom_time: Option<DateTime<Local>>,
}

impl TimeMeasure {
    /// Create a new `TimeMeasure` with the given strftime format string.
    pub fn new(format: &str) -> Self {
        Self {
            format: format.to_string(),
            time_zone: None,
            current_value: MeasureValue::String(String::new()),
            custom_time: None,
        }
    }

    /// Instantiate a `TimeMeasure` from a parsed `MeasureConfig`.
    pub fn from_config(config: &MeasureConfig) -> Self {
        let format = config
            .format
            .as_deref()
            .or_else(|| config.get("format"))
            .unwrap_or("%H:%M:%S")
            .to_string();

        let time_zone = config.get("timezone").map(|s| s.to_string());

        Self {
            format,
            time_zone,
            current_value: MeasureValue::String(String::new()),
            custom_time: None,
        }
    }

    /// Return the configured timezone setting.
    pub fn time_zone(&self) -> Option<&str> {
        self.time_zone.as_deref()
    }

    /// Set timezone setting.
    pub fn set_time_zone(&mut self, tz: Option<String>) {
        self.time_zone = tz;
    }

    /// Provide a custom timestamp (useful for deterministic tests).
    pub fn with_custom_time(mut self, dt: DateTime<Local>) -> Self {
        self.custom_time = Some(dt);
        self
    }

    /// Set or clear the custom timestamp.
    pub fn set_custom_time(&mut self, dt: Option<DateTime<Local>>) {
        self.custom_time = dt;
    }

    /// Update format string dynamically.
    pub fn set_format(&mut self, format: &str) {
        self.format = format.to_string();
    }
}

impl Measure for TimeMeasure {
    fn update(&mut self) -> MeasureValue {
        let base = self.custom_time.unwrap_or_else(Local::now);
        let chrono_fmt = convert_rainmeter_time_format(&self.format);

        let formatted = if let Some(ref tz) = self.time_zone {
            let lower = tz.trim().to_ascii_lowercase();
            if lower == "utc" || lower == "gmt" || lower == "0" {
                let utc_dt = base.naive_utc().and_utc();
                safe_format_time(&utc_dt, &chrono_fmt)
            } else if let Ok(offset_hours) = lower.parse::<f64>() {
                let offset_secs = (offset_hours * 3600.0) as i32;
                if let Some(offset) = chrono::FixedOffset::east_opt(offset_secs) {
                    let dt = base.naive_utc().and_local_timezone(offset).unwrap();
                    safe_format_time(&dt, &chrono_fmt)
                } else {
                    safe_format_time(&base, &chrono_fmt)
                }
            } else {
                safe_format_time(&base, &chrono_fmt)
            }
        } else {
            safe_format_time(&base, &chrono_fmt)
        };

        self.current_value = MeasureValue::String(formatted);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
