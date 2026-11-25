//! The required automatic discharge floor proves these programs with no hint
//! (`wiki/spec/proofs/contracts.md`, "Required automatic discharge"), and
//! each twin differs from its positive in one input the floor depends on.
//!
//! Landing: an operand whose literal bounds meet is that literal ("An
//! operand's bounds are the tightest literal bounds order reasoning gives
//! it"). `(prefix - 496) + a32` equals `a32` because `prefix` is exactly 496;
//! the twin subtracts 495, and `result == a32` is then false.
//!
//! Case splits: "A disjunctive premise whose atoms share a subject with the
//! goal is split". The same contradictory disjunction over `x` proves
//! `10 <= x` but is not split for `10 <= y`.

#[path = "support/source_handoff.rs"]
mod source;

use source::Program;

const LANDED: &str = include_str!("fixtures/required_floor/pass/landed_prefix/main.omg");
const UNLANDED: &str = include_str!("fixtures/required_floor/fail/unlanded_prefix/main.omg");
const SHARED: &str = include_str!("fixtures/required_floor/pass/shared_subject/main.omg");
const UNRELATED: &str = include_str!("fixtures/required_floor/fail/unrelated_subject/main.omg");

/// The program text after its leading comment.
fn body(source: &str) -> &str {
    let start = source.find("machine").unwrap();
    &source[start..]
}

#[test]
fn an_operand_fixed_by_its_bounds_lands_on_its_literal() {
    Program::of(LANDED).judged(|_| {}).accepted();
}

#[test]
fn an_operand_landed_one_away_does_not_prove_the_equality() {
    assert_eq!(
        body(LANDED).replace("(prefix - 496u64)", "(prefix - 495u64)"),
        body(UNLANDED)
    );
    let rejection = Program::of(UNLANDED).judged(|_| {}).rejected();
    // The `ensures` at the one exit is what fails.
    assert_eq!(
        rejection.unproved,
        ["NormalGuarantee { exit: EdgeId(1), clause: 0, published: ObligationId(1) }"],
        "{rejection:?}"
    );
}

#[test]
fn a_disjunction_sharing_the_goals_subject_is_split() {
    Program::of(SHARED).judged(|_| {}).accepted();
}

#[test]
fn a_disjunction_sharing_no_subject_with_the_goal_is_not_split() {
    assert_eq!(
        body(SHARED).replace("x - 10u64", "y - 10u64"),
        body(UNRELATED)
    );
    let rejection = Program::of(UNRELATED).judged(|_| {}).rejected();
    // The subtraction's safety is what fails; the contradictory requirement
    // is not split to discharge it.
    assert_eq!(
        rejection.unproved,
        ["OperationSafety { site: Operation(OperationId(2)) }"],
        "{rejection:?}"
    );
}
