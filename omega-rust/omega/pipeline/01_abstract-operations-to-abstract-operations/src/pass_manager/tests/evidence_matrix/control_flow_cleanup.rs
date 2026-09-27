//! `Optimization::ControlFlowCleanup` evidence matrix legs.

use super::super::verified_linear_empty_block_unit;
use super::{SelectionMatrix, assert_full_entrance_leg};
use crate::rules::LinearEmptyBlockThreadRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    LinearEmptyBlockThreadRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::ControlFlowCleanup,
    expected_rule,
    // Thread the empty block, then merge the now-adjacent entry and target.
    expected_commits: 2,
    positive: verified_linear_empty_block_unit,
    sibling: Optimization::SparseConditionalConstantPropagation,
};

#[test]
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}
