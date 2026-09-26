use crate::tests::front_end::checked_program_result;

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
