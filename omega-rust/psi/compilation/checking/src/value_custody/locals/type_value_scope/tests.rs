fn check(source: &str, accepted: bool) {
    let outcome = crate::validate_program(&crate::front_end::typed_program(source));
    if accepted {
        assert!(outcome.is_ok(), "{outcome:?}\n{source}");
    } else {
        let diagnostics = outcome.expect_err("foreign type-bound value must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit parameter scope")),
            "{diagnostics:?}\n{source}"
        );
    }
}

#[test]
fn local_and_cast_type_bounds_use_the_prior_statement_prefix() {
    for declaration in [
        "let value: f64 [0.0..=limit] = 0.0;",
        "let value: f64 = 0.0 as f64 [0.0..=limit];",
    ] {
        check(
            &format!("machine run() {{ let limit: f64 = 4.0; {declaration} }}"),
            true,
        );
        check(
            &format!("machine run() {{ {declaration} let limit: f64 = 4.0; }}"),
            false,
        );
        check(
            &format!(
                "machine run(limit: f64) {{ transition {{ _ -> next() }} state next() {{ {declaration} }} }}"
            ),
            false,
        );
    }
}

#[test]
fn return_and_nested_element_bounds_use_the_signature_frontier() {
    for (parameters, arguments, bound, accepted) in [
        ("", "", "hidden", false),
        ("limit: f64", "hidden", "limit", true),
    ] {
        check(
            &format!(
                "machine run(hidden: f64) -> f64 {{ transition {{ _ -> next({arguments}) }} state next({parameters}) -> f64 [0.0..={bound}] {{ 0.0 }} }}"
            ),
            accepted,
        );
        let separator = if parameters.is_empty() { "" } else { ", " };
        check(
            &format!(
                "machine run(hidden: f64) {{ transition {{ _ -> next({arguments}{separator}[0.0]) }} state next({parameters}{separator}values: [f64 [0.0..={bound}]; 1]) {{}} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn same_signature_slice_bounds_and_const_array_extents_remain_valid() {
    check(
        "machine run(items: &[u8], index: u64 [0..items.len]) {}",
        true,
    );
    check(
        "machine run(index: u64 [0..items.len], items: &[u8]) {}",
        true,
    );
    check(
        "machine run<const N: u64>(values: &[u8; N]) { transition { _ -> next(values) } state next(items: &[u8; N]) {} }",
        true,
    );
    let outcome = crate::validate_program(&crate::front_end::typed_program(
        "machine run(hidden: u64) { transition { _ -> next() } state next() { let items: [u8; hidden]; } }",
    ));
    assert!(
        outcome
            .expect_err("runtime fixed extent is unsupported")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("fixed-array length"))
    );
}
