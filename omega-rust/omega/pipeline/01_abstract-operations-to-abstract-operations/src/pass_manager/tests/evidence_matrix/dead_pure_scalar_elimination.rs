//! `Optimization::DeadPureScalarElimination` evidence matrix legs.

use super::super::verified_dead_literals_unit;
use super::{SelectionMatrix, assert_corruption_legs};
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
fn forged_run_axes_fail_publication_replay() {
    assert_corruption_legs(&CASE);
}
