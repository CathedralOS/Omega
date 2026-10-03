//! Source checks exercise real invariant demand discovery, not only scratch
//! substitution. Negative programs keep strict ranking but have false ensures.

fn program(
    handoff: &str,
    update: &str,
    entry_requires: &str,
    loop_requires: &str,
    bridge: &str,
) -> String {
    format!(
        r#"
machine descend(n: u64, previous: u64) -> u64
requires {entry_requires}
terminates by n -> Nat::Descending;
ensures result <= previous
{{
    transition n > 1 {{
        true -> descend(n - 1, n)
        false -> {handoff}
    }}
    {bridge}
    state loop(n: u64, p: u64) -> u64
    requires {loop_requires}
    {{
        transition n > 0 {{
            true -> loop(n - 1, {update})
            false -> p
        }}
    }}
}}
"#
    )
}

fn ordinary(handoff: &str, update: &str, bridge: &str) -> String {
    program(
        handoff,
        update,
        "n <= previous && previous <= 1000",
        "n <= p && p <= 1000",
        bridge,
    )
}

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let typed = crate::tests::front_end::typed_program_with_core_service(&format!(
        "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
    ));
    crate::checks::termination::check_machine_termination(&typed).unwrap_or_else(|diagnostics| {
        panic!("both cycles remain strictly ranked: {diagnostics:#?}")
    });
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
}

#[test]
fn relational_components_prove_the_unchanged_accumulator() {
    check(&ordinary("loop(n, previous)", "n", ""))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

#[test]
fn relational_components_transport_compounds_through_an_acyclic_bridge() {
    let bridge = r#"
    state bridge(carried: u64, count: u64) -> u64
    requires count <= carried && carried <= 1000
    {
        transition { _ -> loop(count, carried + 0) }
    }
    "#;
    check(&ordinary("bridge(previous, n)", "n", bridge))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
}

fn rejects_false_guarantee(source: &str) {
    let diagnostics = check(source).expect_err("a false exit guarantee has no proved demand");
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
        "negative must retain valid call premises: {diagnostics:#?}"
    );
}

#[test]
fn relational_components_reject_wrong_downstream_arrival() {
    // descend(3,9) returns 1000; ranking still decreases on both cycles.
    rejects_false_guarantee(&ordinary("loop(n, previous)", "1000", ""));
}

#[test]
fn relational_components_reject_an_unestablished_external_base() {
    // The handoff respects the destination requirements but returns 1000.
    rejects_false_guarantee(&ordinary("loop(0, 1000)", "n", ""));
}

#[test]
fn relational_components_reject_missing_invocation_establishment() {
    // Without n<=previous, descend(2,0) returns 1, contradicting ensures.
    rejects_false_guarantee(&program(
        "loop(n, previous)",
        "n",
        "n <= 1000 && previous <= 1000",
        "n <= 1000 && p <= 1000",
        "",
    ));
}
