use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use crate::variables::VariableMap;
use std::collections::HashMap;

/// Fallback measure for unsupported or environment-specific Rainmeter plugins
/// (such as Chameleon, FrostedGlass, etc.)
#[derive(Debug, Clone)]
pub struct FallbackPluginMeasure {
    name: String,
    #[allow(dead_code)]
    plugin: String,
    substitute: Option<String>,
    regexp_substitute: bool,
    current_value: MeasureValue,
}

impl FallbackPluginMeasure {
    pub fn from_config(config: &MeasureConfig) -> Self {
        let plugin = config
            .plugin
            .clone()
            .or_else(|| config.properties.get("plugin").cloned())
            .unwrap_or_default()
            .to_ascii_lowercase();

        let substitute = config.properties.get("substitute").cloned();
        let regexp_substitute = config
            .properties
            .get("regexpsubstitute")
            .map(|s| s == "1")
            .unwrap_or(false);

        let default_val = match plugin.as_str() {
            "chameleon" => {
                let color_prop = config.get("color").unwrap_or("").to_ascii_lowercase();
                if color_prop == "luminance" {
                    MeasureValue::Number(0.0)
                } else {
                    MeasureValue::String("333333".to_string())
                }
            }
            "frostedglass" => MeasureValue::Number(1.0),
            _ => MeasureValue::Number(0.0),
        };

        Self {
            name: config.name.clone(),
            plugin,
            substitute,
            regexp_substitute,
            current_value: default_val,
        }
    }
}

impl Measure for FallbackPluginMeasure {
    fn update(&mut self) -> MeasureValue {
        self.current_value.clone()
    }

    fn get_value(&self) -> MeasureValue {
        self.current_value.clone()
    }

    fn update_with_context(
        &mut self,
        vars: &VariableMap,
        measures: &HashMap<String, MeasureValue>,
    ) -> MeasureValue {
        if let Some(ref sub) = self.substitute {
            let s = self.current_value.to_string_val();
            let exp_sub = vars.expand_with_context(sub, Some(&self.name), Some(measures));
            let sub_res = super::substitute::apply_substitute(&s, &exp_sub, self.regexp_substitute);
            self.current_value = MeasureValue::String(sub_res);
        }
        self.current_value.clone()
    }
}
