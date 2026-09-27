//! `Optimization::LoopInvariantScalarMotion` evidence matrix legs.
//!
//! The relocation boundary is a session-level pass step, not a leaf-rule
//! registry, but it enters through the same public entrances: its retained
//! commit names the scheduled relocation roster's rule identity.

use super::super::VerifiedPsiOptimizationUnit;
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_negative_leg,
};
use crate::ranked_rewrites::loop_invariant_scalar_motion::rule_identity;
use crate::ranked_rewrites::loop_invariant_scalar_motion::tests::{
    TRANSITIVE_MEMBER_SOURCE, lowered_unit,
};
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    rule_identity()
}

fn positive() -> VerifiedPsiOptimizationUnit {
    lowered_unit(
        TRANSITIVE_MEMBER_SOURCE,
        "loop-invariant relocation matrix",
    )
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::LoopInvariantScalarMotion,
    expected_rule,
    // The transitive member parameter's `s + s` relocates to the component's
    // unique preheader as one committed roster.
    expected_commits: 1,
    positive,
    // The sibling must decline the cyclic unit outright: leaf passes that
    // still propose inside a frozen component meet the control-cycle custody
    // fence as a run error, while state specialization declines frozen
    // territory without nominating.
    sibling: Optimization::StateSpecialization,
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
