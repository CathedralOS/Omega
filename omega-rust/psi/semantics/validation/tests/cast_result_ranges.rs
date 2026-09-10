//! A cast's asserted range is an obligation, never its own proof premise.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

fn diagnostics(source: &str) -> Vec<String> {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = lower_syntax_trees(&syntax).expect("resolution");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing");
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn cast_range_requires_actual_membership_for_every_arithmetic_policy() {
    for policy in ["", " in Wrapping", " in Saturating", " in Trapping"] {
        for value in ["0", "10", "7 / 2 * 2"] {
            let source =
                format!("machine run() -> u64 {{ (({value}) as u64 [0..=10]{policy}) as u64 }}");
            let errors = diagnostics(&source);
            assert!(errors.is_empty(), "{source}: {errors:?}");
        }
        for (parameters, value) in [("", "11"), ("value: u64", "value")] {
            let source = format!(
                "machine run({parameters}) -> u64 {{ ({value} as u64 [0..=10]{policy}) as u64 }}"
            );
            let errors = diagnostics(&source);
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains("cast target range")),
                "{source}: {errors:?}"
            );
        }
    }
}

#[test]
fn cast_ranges_use_live_source_facts_and_do_not_prove_themselves() {
    for source in [
        "machine run(value: u64 [0..=10]) -> u64 { value as u64 [0..=10] }",
        "machine run(value: u64) -> u64 requires value <= 10 { value as u64 [0..=10] }",
        "machine run(value: u64) -> u64 { transition value <= 10 { true -> (value as u64 [0..=10]) false -> 0 } }",
        "machine run(value: u64) -> u64 { transition value > 10 { true -> 0 false -> (value as u64 [0..=10]) } }",
    ] {
        let errors = diagnostics(source);
        assert!(errors.is_empty(), "{source}: {errors:?}");
    }
    for source in [
        "machine run(value: u64) -> u64 { value as u64 [0..=10] }",
        "machine run(value: u64) -> u64 { (value as u64 [0..=10]) as u64 [0..=10] }",
        "machine change(value: &mut u64) { value = 100; } machine run(value: &mut u64) -> u64 requires value <= 10 { change(value); value as u64 [0..=10] }",
    ] {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cast target range")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn cast_ranges_validate_nested_and_selected_results() {
    for expression in [
        "(11 as u64 [0..=10]) + 1u64",
        "match true { true -> 11 as u64 [0..=10], false -> 1 as u64 [0..=10] }",
        "(11 as u64 [0..=10]) as u64",
    ] {
        let source = format!("machine run() -> u64 {{ {expression} }}");
        let errors = diagnostics(&source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cast target range")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn cast_range_uses_each_operations_policy_result() {
    for source in [
        "machine run() -> u8 { ((((255u8 as u8 in Saturating) + 1) - 1) as u8 [255..=255]) as u8 }",
        "machine run() -> u8 { ((((255u8 as u8 in Wrapping) + 1) / 2) as u8 [128..=128]) as u8 }",
    ] {
        let errors = diagnostics(source);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("cast target range")),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn float_cast_ranges_use_live_bounds_without_asserting_their_own_membership() {
    let source =
        "machine run() { let limit: f64 = 4.0; let value: f64 = 0.0 as f64 [0.0..=limit]; }";
    assert!(diagnostics(source).is_empty());
    for source in [
        "machine run() -> f64 { 11.0 as f64 [0.0..=10.0] }",
        "machine run(value: f64) -> f64 { value as f64 [0.0..=10.0] }",
        "machine change(value: &mut f64) { value = 100.0; } machine run() -> f64 { let value: f64 = 1.0; change(&mut value); value as f64 [0.0..=10.0] }",
        "machine run(other: f64) -> f64 { let value: f64 = 1.0; value = other; value as f64 [0.0..=10.0] }",
    ] {
        let errors = diagnostics(source);
        assert_eq!(
            errors
                .iter()
                .filter(|error| error.contains("cast target range"))
                .count(),
            1,
            "{source}: {errors:?}"
        );
    }
}
