use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use chrono::{DateTime, Local};

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

        // Convert Windows-style non-padded specifiers (%#d, %#H, %#I, %#m, etc.) to chrono (%-d, %-H, etc.)
        let chrono_fmt = self.format.replace("%#", "%-");

        let formatted = if let Some(ref tz) = self.time_zone {
            let lower = tz.trim().to_ascii_lowercase();
            if lower == "utc" || lower == "gmt" || lower == "0" {
                let utc_dt = base.naive_utc().and_utc();
                utc_dt.format(&chrono_fmt).to_string()
            } else if let Ok(offset_hours) = lower.parse::<f64>() {
                let offset_secs = (offset_hours * 3600.0) as i32;
                if let Some(offset) = chrono::FixedOffset::east_opt(offset_secs) {
                    let dt = base.naive_utc().and_local_timezone(offset).unwrap();
                    dt.format(&chrono_fmt).to_string()
                } else {
                    base.format(&chrono_fmt).to_string()
                }
            } else {
                base.format(&chrono_fmt).to_string()
            }
        } else {
            base.format(&chrono_fmt).to_string()
        };

        self.current_value = MeasureValue::String(formatted);
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }
}
