use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use crate::variables::VariableMap;
use std::collections::HashMap;

/// Measure of type `String`. Evaluates a string and optional substitutions.
#[derive(Debug, Clone)]
pub struct StringMeasure {
    name: String,
    raw_string: String,
    substitute: Option<String>,
    regexp_substitute: bool,
    dynamic_variables: bool,
    current_value: MeasureValue,
}

impl StringMeasure {
    pub fn from_config(config: &MeasureConfig) -> Self {
        let raw_string = config
            .properties
            .get("string")
            .cloned()
            .unwrap_or_default();
        let substitute = config.properties.get("substitute").cloned();
        let regexp_substitute = config
            .properties
            .get("regexpsubstitute")
            .map(|s| s == "1")
            .unwrap_or(false);
        let dynamic_variables = config.dynamic_variables
            || config
                .properties
                .get("dynamicvariables")
                .map(|s| s == "1")
                .unwrap_or(false);

        let initial_val = if let Some(ref sub) = substitute {
            super::substitute::apply_substitute(&raw_string, sub, regexp_substitute)
        } else {
            raw_string.clone()
        };

        Self {
            name: config.name.clone(),
            raw_string,
            substitute,
            regexp_substitute,
            dynamic_variables,
            current_value: MeasureValue::String(initial_val),
        }
    }
}

impl Measure for StringMeasure {
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
        let expanded = if self.dynamic_variables {
            vars.expand_with_context(
                &self.raw_string,
                Some(&self.name),
                Some(measures),
            )
        } else {
            vars.expand(&self.raw_string)
        };

        let final_val = if let Some(ref sub) = self.substitute {
            let exp_sub = vars.expand_with_context(sub, Some(&self.name), Some(measures));
            super::substitute::apply_substitute(&expanded, &exp_sub, self.regexp_substitute)
        } else {
            expanded
        };

        self.current_value = MeasureValue::String(final_val);
        self.current_value.clone()
    }
}
