//! `Optimization::DeadPureScalarElimination` evidence matrix legs.

use super::super::verified_dead_literals_unit;
use super::{
    SelectionMatrix, assert_corruption_legs, assert_disabled_leg,
    assert_empty_selection_leg, assert_full_entrance_leg, assert_negative_leg,
};
use crate::rules::DeadScalarLiteralEliminationRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    DeadScalarLiteralEliminationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::DeadPureScalarElimination,
    expected_rule,
    // One commit per unused literal.
    expected_commits: 2,
    positive: verified_dead_literals_unit,
    sibling: Optimization::CopyPropagation,
};

#[test]
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}

#[test]
fn disabled_sibling_selection_leaves_the_unit_untouched() {
    assert_disabled_leg(&CASE);
}

#[test]
fn empty_selection_leaves_the_unit_untouched() {
    assert_empty_selection_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn forged_run_axes_fail_publication_replay() {
    assert_corruption_legs(&CASE);
}
