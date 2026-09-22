use validation::validate_program;

fn diagnostics(expression: &str) -> Vec<String> {
    let source = format!(
        "machine need(flag: bool) -> bool requires flag == true {{ true }}
         machine run(gate: bool) {{ let ignored: bool = {expression}; }}"
    );
    let typed = crate::front_end::typed_program(&source);
    validate_program(&typed)
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn skipped_value_calls_do_not_require_unexecuted_preconditions() {
    for expression in [
        "false && need(false)",
        "true || need(false)",
        "false && need(need(false))",
        "true || (gate && need(false))",
        "true && need(true)",
        "false || need(true)",
    ] {
        let diagnostics = diagnostics(expression);
        assert!(diagnostics.is_empty(), "{expression}: {diagnostics:?}");
    }
}

#[test]
fn reachable_value_calls_still_reject_refuted_preconditions() {
    for expression in [
        "true && need(false)",
        "false || need(false)",
        "gate && need(false)",
        "gate || need(false)",
        "need(false) && false",
        "need(false) || true",
        "false || (true && need(false))",
    ] {
        let diagnostics = diagnostics(expression);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains("violates required fact")),
            "{expression}: {diagnostics:?}"
        );
    }
}

#[test]
fn skipped_value_calls_still_validate_scope_types_and_arity() {
    for (expression, expected) in [
        ("false && need(missing)", "uses `missing`"),
        ("true || need()", "expects 1 argument"),
        ("false && need(7)", "bool"),
    ] {
        let diagnostics = diagnostics(expression);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.contains(expected)),
            "{expression}: {diagnostics:?}"
        );
    }
}
