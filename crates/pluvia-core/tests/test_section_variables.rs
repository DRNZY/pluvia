use pluvia_core::formulas::eval_formula;
use pluvia_core::render::Rect;
use pluvia_core::variables::VariableMap;
use std::collections::HashMap;

#[test]
fn test_section_variable_meter_dimensions_expansion() {
    let mut vars = VariableMap::new();
    vars.set("Bands", "10");
    vars.set("MagicNumber", "-10");

    let mut bounds = HashMap::new();
    bounds.insert("amogus".to_string(), Rect::new(50.0, 100.0, 120.0, 80.0));
    bounds.insert("grid".to_string(), Rect::new(0.0, 0.0, 40.0, 30.0));

    let formula_str = "((#Bands# * [amogus:W]) - ([grid:W] + #MagicNumber#)) / #Bands#";
    let expanded = vars.expand_with_full_context(formula_str, None, None, Some(&bounds));

    assert_eq!(expanded, "((10 * 120) - (40 + -10)) / 10");

    let result = eval_formula(&expanded, &vars).expect("Formula evaluation should succeed");
    // (1200 - 30) / 10 = 1170 / 10 = 117.0
    assert_eq!(result, 117.0);
}

#[test]
fn test_section_variable_coordinates() {
    let vars = VariableMap::new();
    let mut bounds = HashMap::new();
    bounds.insert("ClockMeter".to_string(), Rect::new(150.0, 250.0, 300.0, 100.0));

    let expanded_x = vars.expand_with_full_context("[ClockMeter:X] + 10", None, None, Some(&bounds));
    assert_eq!(expanded_x, "150 + 10");

    let expanded_y = vars.expand_with_full_context("[ClockMeter:Y] + 20", None, None, Some(&bounds));
    assert_eq!(expanded_y, "250 + 20");
}
