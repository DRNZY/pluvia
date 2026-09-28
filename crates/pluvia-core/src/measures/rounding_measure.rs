use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use crate::variables::VariableMap;
use std::collections::HashMap;

/// Rainmeter `RoundingMeasure`: wraps a parent measure and formats its numeric value
/// to a fixed number of decimals.
///
/// Skins normally write:
/// ```ini
/// [MeasureRoundedRAM]
/// Measure=RoundingMeasure
/// MeasureName=MeasureRAM
/// Rounding=0
/// ```
/// and then reference `[MeasureRoundedRAM]` in meters. Without this, a percentage such
/// as `29.806855367393485` leaks full f64 precision into the UI.
#[derive(Debug, Clone)]
pub struct RoundingMeasure {
    parent: Option<String>,
    rounding: Option<usize>,
    format: Option<String>,
    current_value: MeasureValue,
}

impl RoundingMeasure {
    pub fn from_config(config: &MeasureConfig) -> Self {
        // `MeasureName` is inherited from the parent, but be permissive and also accept
        // the common `MeasureName`/`Parent` spellings used by third-party wrappers.
        let parent = config
            .get("measurename")
            .or_else(|| config.get("parent"))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let rounding = config
            .get("rounding")
            .and_then(|s| s.trim().parse::<i64>().ok())
            .map(|n| n.clamp(0, 15) as usize);

        let format = config
            .format
            .clone()
            .or_else(|| config.get("format").map(str::to_string))
            .filter(|s| !s.trim().is_empty());

        Self {
            parent,
            rounding,
            format,
            current_value: MeasureValue::Number(0.0),
        }
    }

    fn apply(&self, value: &MeasureValue) -> MeasureValue {
        match value {
            MeasureValue::String(s) => {
                // A parent that yields text passes through unchanged, unless the skin
                // supplied an explicit `Format` pattern.
                match &self.format {
                    Some(fmt) => {
                        let n = s.trim().parse::<f64>().unwrap_or(0.0);
                        MeasureValue::String(format_value(n, Some(fmt), self.rounding))
                    }
                    None => value.clone(),
                }
            }
            MeasureValue::Number(n) => MeasureValue::String(format_value(
                *n,
                self.format.as_deref(),
                self.rounding,
            )),
        }
    }
}

/// Renders a float using Rainmeter's formatting rules.
fn format_value(value: f64, format: Option<&str>, rounding: Option<usize>) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    match format {
        Some(fmt) => {
            let decimals = rounding.unwrap_or_else(|| default_decimals(fmt));
            format!("{:.*}", decimals, value)
        }
        None => {
            let decimals = rounding.unwrap_or(2);
            let s = format!("{:.*}", decimals, value);
            // Trim trailing zeros that carry no information ("29.80" -> "29.8",
            // "30.00" -> "30") so widgets stay compact.
            if decimals > 0 && s.contains('.') {
                let trimmed = s.trim_end_matches('0');
                let trimmed = trimmed.trim_end_matches('.');
                if trimmed.is_empty() || trimmed == "-" {
                    "0".to_string()
                } else {
                    trimmed.to_string()
                }
            } else {
                s
            }
        }
    }
}

/// Derives a decimal count from a printf-style pattern like `%.2f` or `%.0f`.
fn default_decimals(format: &str) -> usize {
    if let Some(idx) = format.find('.') {
        let digits: String = format[idx + 1..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        return digits.parse().unwrap_or(0);
    }
    0
}

impl Measure for RoundingMeasure {
    fn update(&mut self) -> MeasureValue {
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn update_with_context(
        &mut self,
        _vars: &VariableMap,
        measures: &HashMap<String, MeasureValue>,
    ) -> MeasureValue {
        let raw = self
            .parent
            .as_ref()
            .and_then(|p| measures.get(&p.to_ascii_lowercase()))
            .or_else(|| {
                self.parent
                    .as_ref()
                    .and_then(|p| measures.get(p))
            });

        match raw {
            Some(v) => {
                let out = self.apply(v);
                self.current_value = out.clone();
                out
            }
            None => {
                let out = MeasureValue::Number(0.0);
                self.current_value = out.clone();
                out
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ini::MeasureConfig;
    use std::collections::HashMap;

    fn cfg(rounding: Option<&str>) -> MeasureConfig {
        let mut props = HashMap::new();
        props.insert("measurename".to_string(), "parent".to_string());
        if let Some(r) = rounding {
            props.insert("rounding".to_string(), r.to_string());
        }
        MeasureConfig {
            name: "rounded".to_string(),
            measure_type: "RoundingMeasure".to_string(),
            plugin: None,
            format: None,
            formula: None,
            update_divider: 1,
            disabled: false,
            dynamic_variables: false,
            properties: props,
        }
    }

    fn parent_map(v: MeasureValue) -> HashMap<String, MeasureValue> {
        let mut m = HashMap::new();
        m.insert("parent".to_string(), v);
        m
    }

    #[test]
    fn rounds_to_zero_decimals() {
        let mut m = RoundingMeasure::from_config(&cfg(Some("0")));
        let out = m.update_with_context(
            &VariableMap::new(),
            &parent_map(MeasureValue::Number(29.806_855_367_393_485)),
        );
        assert_eq!(out.to_string_val(), "30");
    }

    #[test]
    fn rounds_to_two_decimals_trimming_zeros() {
        let mut m = RoundingMeasure::from_config(&cfg(Some("2")));
        let out = m.update_with_context(
            &VariableMap::new(),
            &parent_map(MeasureValue::Number(29.806_855_367_393_485)),
        );
        assert_eq!(out.to_string_val(), "29.81");
    }

    #[test]
    fn trims_trailing_zeros() {
        let mut m = RoundingMeasure::from_config(&cfg(Some("2")));
        let out = m.update_with_context(
            &VariableMap::new(),
            &parent_map(MeasureValue::Number(30.0)),
        );
        assert_eq!(out.to_string_val(), "30");
    }

    #[test]
    fn missing_parent_yields_zero() {
        let mut m = RoundingMeasure::from_config(&cfg(Some("0")));
        let out = m.update_with_context(&VariableMap::new(), &HashMap::new());
        assert_eq!(out.to_number_val(), 0.0);
    }

    #[test]
    fn string_parent_passes_through() {
        let mut m = RoundingMeasure::from_config(&cfg(Some("0")));
        let out = m.update_with_context(
            &VariableMap::new(),
            &parent_map(MeasureValue::String("hello".to_string())),
        );
        assert_eq!(out.to_string_val(), "hello");
    }
}
