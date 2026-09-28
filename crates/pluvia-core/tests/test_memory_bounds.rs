//! Memory-safety regression tests.
//!
//! The monstercat fixture uses `Substitute="":"#Color#"`. An empty search key matches at
//! every character boundary, so `String::replace` inserted the replacement between every
//! character, roughly doubling the string on each tick. After a handful of ticks the
//! runtime attempted a single 3.5 GB allocation and the kernel OOM-killed the process on
//! a 16 GB machine.
//!
//! These tests pin the bound so the behaviour cannot silently regress.

use pluvia_core::measures::substitute::apply_substitute;
use pluvia_core::measures::create_measure;
use pluvia_core::variables::VariableMap;
use std::collections::HashMap;

#[test]
fn empty_substitute_key_is_a_no_op() {
    let sub = "\"\":\"255,255,255,255\"";
    let input = "255,255,255,255";
    assert_eq!(apply_substitute(input, sub, false), input);
}

#[test]
fn empty_substitute_key_stays_bounded_over_many_ticks() {
    let sub = "\"\":\"#Color#\"";
    let mut value = "0123456789".to_string();
    for _ in 0..10_000 {
        value = apply_substitute(&value, sub, false);
    }
    assert!(
        value.len() < 1024,
        "value grew unbounded to {} bytes across 10k ticks",
        value.len()
    );
}

#[test]
fn self_referential_substitute_is_bounded() {
    // "a" -> "ab" would re-introduce the key on every pass.
    let out = apply_substitute("a", "\"a\":\"ab\"", false);
    assert!(out.len() < 1 << 20, "expanding substitute grew to {}", out.len());
}

#[test]
fn repeated_measure_updates_do_not_blow_up_memory() {
    // Mirrors the real failure: a Plugin measure with `Substitute="":"#Color#"` that is
    // re-evaluated on every tick.
    let mut props = HashMap::new();
    props.insert("plugin".to_string(), "Chameleon".to_string());
    props.insert("substitute".to_string(), "\"\":\"#Color#\"".to_string());
    let config = pluvia_core::ini::MeasureConfig {
        name: "MeasureCoverColor".to_string(),
        measure_type: "Plugin".to_string(),
        plugin: Some("Chameleon".to_string()),
        format: None,
        formula: None,
        update_divider: 1,
        disabled: false,
        dynamic_variables: false,
        properties: props,
    };

    let mut measure = create_measure(&config).expect("measure should be constructible");
    let vars = VariableMap::new();
    for tick in 0..1_000 {
        let value = measure.update_with_context(&vars, &HashMap::new());
        let len = value.to_string_val().len();
        assert!(
            len < 1 << 16,
            "measure value grew to {len} bytes at tick {tick} - exponential growth"
        );
    }
}
