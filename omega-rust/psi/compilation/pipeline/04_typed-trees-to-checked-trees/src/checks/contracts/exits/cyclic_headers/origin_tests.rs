//! Origin loss must not be hidden by another parameter's exact forwarding.

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let program = crate::tests::front_end::typed_program_with_core_service(&format!(
        "boundary trait MachineControl {{}}\nboundary trait PortIo {{}}\n{source}"
    ));
    crate::checks::termination::check_machine_termination(&program)
        .unwrap_or_else(|diagnostics| panic!("ranking remains lawful: {diagnostics:#?}"));
    crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
}

fn sequential(entry_previous: &str, handoff_previous: &str) -> String {
    format!(
        r#"
machine descend(n: u64 [0..=1000], previous: u64 [0..=1000]) -> u64
terminates by n -> Nat::Descending;
ensures result <= previous
{{
    transition n > 0 {{
        true -> descend(n - 1, {entry_previous})
        false -> loop(n, {handoff_previous})
    }}
    state loop(n: u64 [0..=1000], p: u64 [0..=1000]) -> u64 {{
        transition n > 0 && n <= p {{
            true -> loop(n - 1, n)
            false -> p
        }}
    }}
}}
"#
    )
}

fn rejects_false_guarantee(source: &str) {
    let diagnostics =
        check(source).expect_err("origin substitution must not prove a false guarantee");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}

#[test]
fn sequential_origins_reject_changed_entry_previous() {
    rejects_false_guarantee(&sequential("1000", "previous"));
}

#[test]
fn sequential_origins_reject_substituted_handoff_previous() {
    rejects_false_guarantee(&sequential("previous", "1000"));
}

#[test]
fn sequential_origins_reject_swapped_carried_values() {
    rejects_false_guarantee(
        r#"
machine descend(n: u64 [0..=1000], previous: u64 [0..=1000], other: u64 [0..=1000]) -> u64
terminates by n -> Nat::Descending;
ensures result <= previous
{
    transition n > 0 {
        true -> descend(n - 1, other, previous)
        false -> loop(n, previous)
    }
    state loop(n: u64 [0..=1000], p: u64 [0..=1000]) -> u64 {
        transition n > 0 && n <= p {
            true -> loop(n - 1, n)
            false -> p
        }
    }
}
"#,
    );
}
