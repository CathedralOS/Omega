use crate::tests::front_end::checked_program_result;

#[test]
fn selected_expression_operator_cannot_drop_its_crash_contract() {
    for format in ["i32", "f32", "f64"] {
        for (operator_contract, caller_contract) in [
            ("crashes Trap", ""),
            ("crashes Abort", ""),
            ("crashes Trap", "crashes Abort"),
            ("crashes Abort", "crashes Trap"),
        ] {
            let source = format!(
                "boundary operator == Comparison::equal(left: {format}, right: {format}) -> bool {operator_contract};
                 pub machine compare(left: {format}, right: {format}) -> bool {caller_contract} {{ left == right }}"
            );
            let diagnostics = checked_program_result(&source).err().unwrap_or_else(|| {
                panic!("selected operator crash contract disappeared: {source}")
            });
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("uncovered")),
                "{source}\n{diagnostics:#?}"
            );
        }
    }
}

#[test]
fn crash_free_expression_operator_remains_accepted() {
    for format in ["i32", "f32", "f64"] {
        checked_program_result(&format!(
            "boundary operator == Comparison::equal(left: {format}, right: {format}) -> bool;
             machine compare(left: {format}, right: {format}) -> bool {{ left == right }}"
        ))
        .expect("a crash-free selected comparison owes no crash invocation");
    }
}

#[test]
fn unselected_crash_qualified_overload_does_not_create_an_invocation() {
    checked_program_result(
        "boundary operator == Float::equal(left: f32, right: f32) -> bool crashes Trap;
         boundary operator == Float::equal(left: f64, right: f64) -> bool;
         machine compare(left: f64, right: f64) -> bool { left == right }
         machine integer_compare(left: u64, right: u64) -> bool { left == right }",
    )
    .expect("neither the f64 overload nor builtin equality selects the f32 crash contract");
}

#[test]
fn named_call_to_crash_qualified_operator_cannot_drop_its_contract() {
    for (operator_contract, caller_contract) in [
        ("crashes Trap", ""),
        ("crashes Abort", ""),
        ("crashes Trap", "crashes Abort"),
        ("crashes Abort", "crashes Trap"),
    ] {
        let source = format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool {operator_contract};
             pub machine compare(left: i32, right: i32) -> bool {caller_contract} {{ Comparison::equal(left, right) }}"
        );
        let diagnostics = checked_program_result(&source)
            .err()
            .unwrap_or_else(|| panic!("named operator crash contract disappeared: {source}"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("uncovered")),
            "{source}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn covered_named_call_retains_an_exact_named_crash_site() {
    let checked = checked_program_result(
        "boundary operator == Comparison::equal(left: i32, right: i32) -> bool crashes Trap;
         pub machine compare(left: i32, right: i32) -> bool crashes Trap { Comparison::equal(left, right) }",
    )
    .expect("a covered named operator crash invocation checks");
    assert!(
        checked
            .facts
            .operators
            .has_crash_qualified_uses(&checked.typed)
    );
    let sites = checked
        .facts
        .contract_plans
        .machines
        .iter()
        .flat_map(|machine| machine.crash.checked_operators())
        .collect::<Vec<_>>();
    let [site] = sites.as_slice() else {
        panic!("one named operator crash site: {sites:#?}")
    };
    // The named call keeps its own use identity; it borrows neither a spelled
    // use row nor a flow invocation capture.
    assert!(site.named_use.is_valid());
    assert!(!site.operator_use.is_valid());
    assert!(!site.invocation.is_valid());
    assert_eq!(site.published, site.surviving);
    assert!(!site.published.is_empty());
    let named = checked.facts.operators.named_uses.get(site.named_use);
    assert_eq!(named.selected_operator_symbol, site.selected_operator);
}

#[test]
fn named_call_crash_site_substitutes_exact_entry_operands() {
    for (guard, accepted) in [("right < 0", true), ("left < 0", false)] {
        let checked = checked_program_result(&format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool
             crashes Trap left < 0;
             pub machine compare(left: i32, right: i32) -> bool crashes Trap {guard} {{
                 Comparison::equal(right, left)
             }}"
        ));
        assert_eq!(checked.is_ok(), accepted, "{guard}: {:?}", checked.err());
    }
}
