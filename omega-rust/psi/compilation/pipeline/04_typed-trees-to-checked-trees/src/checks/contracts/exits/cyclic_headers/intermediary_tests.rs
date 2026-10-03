//! An exitless state inside a cycle needs a transported demand. Ranking alone
//! never grants it, and every candidate still needs invocation establishment.

fn source(next_previous: &str, entry_requires: &str, loop_requires: &str) -> String {
    format!(
        r#"
machine descend(n: u64, previous: u64) -> u64
requires {entry_requires}
terminates by n -> Nat::Descending;
ensures result <= previous
{{
    transition n > 1 {{
        true -> carry(n - 1, n)
        false -> loop(n, previous)
    }}
    state carry(n: u64, p: u64) -> u64
    requires n > 0 && n <= p && p <= 1000
    {{
        transition {{ _ -> descend(n - 1, {next_previous}) }}
    }}
    state loop(n: u64, p: u64) -> u64
    requires {loop_requires}
    {{
        transition n > 0 {{
            true -> loop(n - 1, n)
            false -> p
        }}
    }}
}}
"#
    )
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let typed = crate::tests::front_end::typed_program_with_core_service(&format!(
        "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
    ));
    crate::checks::termination::check_machine_termination(&typed).unwrap_or_else(|diagnostics| {
        panic!("all three cyclic edges remain strict: {diagnostics:#?}")
    });
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
}

#[test]
fn intermediary_demands_prove_the_unchanged_ranked_program() {
    check(&source(
        "p",
        "n <= previous && previous <= 1000",
        "n <= p && p <= 1000",
    ))
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

fn rejects_false_guarantee(source: &str) {
    let diagnostics = check(source).expect_err("a false guarantee must not borrow its own demand");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "negative must retain call preconditions: {diagnostics:#?}"
    );
}

#[test]
fn intermediary_demands_reject_wrong_carry_value_with_strict_ranking() {
    // descend(2,9) -> carry(1,2) -> descend(0,1000) returns 1000.
    rejects_false_guarantee(&source(
        "1000",
        "n <= previous && previous <= 1000",
        "n <= p && p <= 1000",
    ));
}

#[test]
fn intermediary_demands_reject_missing_invocation_base() {
    // descend(2,0) -> carry(1,2) -> descend(0,2) returns 2 > 0.
    rejects_false_guarantee(&source(
        "p",
        "n <= 1000 && previous <= 1000",
        "n <= 1000 && p <= 1000",
    ));
}
