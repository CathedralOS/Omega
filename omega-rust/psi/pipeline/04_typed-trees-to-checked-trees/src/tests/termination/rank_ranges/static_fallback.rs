//! Static membership and relational descent remain separate obligations.

use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const DESCENDING: &str =
    "machine walk(n: u32 [0..=10]) terminates by n -> Nat::Descending in 0..=10; -> u32";
fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {diagnostics:#?}\n{source}"));
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("complete checking: {diagnostics:#?}\n{source}"));
}

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err("membership alone must not authorize descent");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove the `terminates by` ranking")
                || diagnostic.message.contains("cannot prove rank range")
        }),
        "{diagnostics:#?}\n{source}"
    );
}

#[test]
fn static_membership_can_consume_a_relational_two_step_decrease() {
    prove(&format!(
        "{DESCENDING} {{ transition n >= 2 {{ true -> walk(n - 2) false -> n }} }}"
    ));
}

#[test]
fn static_membership_does_not_authorize_stalled_or_increasing_rank() {
    for argument in ["n", "n + 1"] {
        reject(&format!(
            "{DESCENDING} {{ transition n >= 2 {{ true -> walk({argument}) false -> n }} }}"
        ));
    }
}

#[test]
fn the_relational_fallback_does_not_reinterpret_authored_arithmetic() {
    for operator in [
        "operator - u32::subtract(left: u32, right: u32) -> u32;",
        "operator >= u32::compare(left: u32, right: u32) -> bool;",
    ] {
        reject(&format!(
            "{operator} {DESCENDING} {{ transition n >= 2 {{ true -> walk(n - 2) false -> n }} }}"
        ));
    }
}

