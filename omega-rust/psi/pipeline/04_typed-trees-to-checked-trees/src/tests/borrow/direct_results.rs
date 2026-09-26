use super::checks::check_program;

fn rejects(source: &str, expected: &str) {
    let diagnostics = check_program(source).expect_err(expected);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "{expected}: {diagnostics:#?}"
    );
}

#[test]
fn every_candidate_source_must_supply_the_direct_results_access() {
    for (source_access, result_access) in [
        ("", "mut "),
        ("", "write "),
        ("write ", ""),
        ("write ", "mut "),
    ] {
        // The body selects the mutable field. The same-lifetime restricted
        // sibling must still prevent promoting any possible input source.
        let source = format!(
            "data Input<'source> {{ restricted: &'source {source_access}i32; selected: &'source mut i32; }}
             machine select<'source>(value: Input<'source>) -> &'source {result_access}i32 {{
                 value.selected
             }}"
        );
        rejects(&source, "access cannot be supplied");
    }
}
