//! A cast's asserted range is an obligation, never its own proof premise.

fn diagnostics(source: &str) -> Vec<String> {
    let typed = crate::front_end::typed_program(source);
    validation::validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
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
