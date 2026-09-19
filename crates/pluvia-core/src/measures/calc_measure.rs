use crate::formulas::eval_formula;
use crate::ini::MeasureConfig;
use crate::measures::{Measure, MeasureValue};
use crate::variables::VariableMap;
use std::collections::HashMap;

/// Measure of type `Calc`. Evaluates arithmetic expressions and optional substitutions.
#[derive(Debug, Clone)]
pub struct CalcMeasure {
    name: String,
    formula: String,
    substitute: Option<String>,
    regexp_substitute: bool,
    dynamic_variables: bool,
    current_value: MeasureValue,
}

impl CalcMeasure {
    pub fn from_config(config: &MeasureConfig) -> Self {
        let formula = config
            .formula
            .clone()
            .or_else(|| config.properties.get("formula").cloned())
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

        Self {
            name: config.name.clone(),
            formula,
            substitute,
            regexp_substitute,
            dynamic_variables,
            current_value: MeasureValue::Number(0.0),
        }
    }
}

impl Measure for CalcMeasure {
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
                &self.formula,
                Some(&self.name),
                Some(measures),
            )
        } else {
            vars.expand(&self.formula)
        };

        let num_res = eval_formula(&expanded, vars)
            .ok()
            .or_else(|| expanded.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        if let Some(ref sub) = self.substitute {
            let num_str = num_res.to_string();
            let exp_sub = vars.expand_with_context(sub, Some(&self.name), Some(measures));
            let sub_res = super::substitute::apply_substitute(&num_str, &exp_sub, self.regexp_substitute);
            self.current_value = MeasureValue::String(sub_res);
        } else {
            self.current_value = MeasureValue::Number(num_res);
        }

        self.current_value.clone()
    }
}
