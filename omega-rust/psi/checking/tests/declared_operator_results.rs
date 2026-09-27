//! Operand-directed selection retains the declaration's result, not an operand carrier.

const SUM: &str = "operator + u8::sum(left: u8, right: u8) -> u64;";

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
fn participating_ambiguous_declarations_cannot_supply_a_result() {
    let source = format!(
        "{SUM} operator + u8::another(left: u8, right: u8) -> u64;
         machine run(left: u8, right: u8) -> u64 {{ left + right }}"
    );
    let errors = diagnostics(&source);
    assert!(
        errors
            .iter()
            .any(|error| error.contains("ambiguous") || error.contains("duplicate")),
        "{source}: {errors:?}"
    );
}

#[test]
fn dependent_declared_result_does_not_rebind_formals_to_caller_names() {
    let source = "operator + u8::bounded(left: u8, right: u8) -> u64 [0..=left];
        machine run(value: u8) -> u64 { let left: u64 = 0; (value + value) as u64 [0..=left] }";
    let errors = diagnostics(source);
    assert!(
        !errors.is_empty(),
        "uninstantiated result predicate became caller evidence"
    );
}
