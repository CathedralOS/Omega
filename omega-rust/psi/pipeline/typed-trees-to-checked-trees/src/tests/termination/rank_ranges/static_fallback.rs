//! Static membership and relational descent remain separate obligations.

use super::{lower_typed_trees, typed};

const DESCENDING: &str =
    "machine walk(n: u32 [0..=10]) terminates by n -> Nat::Descending in 0..=10; -> u32";
const INCREASING: &str = "machine climb(limit: u32 [0..=10], index: u32 [0..=10]) requires index <= limit; terminates by index -> Nat::IncreasingTo(limit) in 0..=limit; -> u32";

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {diagnostics:#?}\n{source}"));
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("complete checking: {diagnostics:#?}\n{source}"));
}

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
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
fn static_view_ceiling_can_consume_a_relational_two_step_increase() {
    // Supply the separate storage-range checker its explicit upper guard;
    // this test concerns the two-step ranking judgment.
    prove(&format!(
        "{INCREASING} {{ transition index <= 8 && index + 1 < limit {{ true -> climb(limit, index + 2) false -> index }} }}"
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
fn the_actual_step_must_preserve_the_natural_rank_floor() {
    reject(&format!(
        "{DESCENDING} {{ transition n > 0 {{ true -> walk(n - 2) false -> n }} }}"
    ));
}

#[test]
fn every_alternative_edge_requires_its_own_decrease() {
    reject(&format!(
        "{DESCENDING} {{ transition {{ n == 2 -> walk(n) n >= 2 -> walk(n - 2) _ -> n }} }}"
    ));
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

#[test]
fn a_smaller_distance_does_not_authorize_a_changed_view_bound() {
    reject(&format!(
        "{INCREASING} {{ transition index + 1 < limit && limit < 10 {{ true -> climb(limit + 1, index + 2) false -> index }} }}"
    ));
}

#[test]
fn mutable_prefixes_and_named_state_arrivals_remain_outside_this_fallback() {
    reject(&format!(
        "{} {{ n = 8; transition n >= 2 {{ true -> walk(n - 2) false -> n }} }}",
        DESCENDING.replace("n: u32", "mut n: u32")
    ));
    reject(&format!(
        "{DESCENDING} {{ transition {{ _ -> step(n) }} state step(remaining: u32 [0..=10]) {{ transition remaining >= 2 {{ true -> walk(remaining - 2) false -> remaining }} }} }}"
    ));
}
