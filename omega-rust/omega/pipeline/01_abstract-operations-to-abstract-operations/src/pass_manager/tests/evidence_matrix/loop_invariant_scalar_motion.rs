//! `Optimization::LoopInvariantScalarMotion` evidence matrix legs.
//!
//! The relocation boundary is a session-level pass step, not a leaf-rule
//! registry, but it enters through the same public entrances: its retained
//! commit names the scheduled relocation roster's rule identity.

use super::super::VerifiedPsiOptimizationUnit;
use super::{
    SelectionMatrix, assert_boundary_leg, assert_composition_leg, assert_corruption_legs,
    assert_determinism_leg, assert_disabled_leg, assert_downstream_replay_leg,
    assert_empty_selection_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_stale_evidence_leg,
};
use crate::ranked_rewrites::loop_invariant_scalar_motion::rule_identity;
use crate::ranked_rewrites::loop_invariant_scalar_motion::tests::{
    MULTI_SOURCE_ENTRY_SOURCE, TRANSITIVE_MEMBER_SOURCE, lowered_unit,
};
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    rule_identity()
}

fn positive() -> VerifiedPsiOptimizationUnit {
    lowered_unit(TRANSITIVE_MEMBER_SOURCE, "loop-invariant relocation matrix")
}

/// A cyclic component whose entries depart different blocks: member
/// computations exist, but there is no shared preheader for the relocation
/// to land in, so the boundary declines the component outright. Note scalar
/// constants are exempt leaves — a variant-operand member alone does not
/// reach a clean decline — so the admission edge is the preheader gate.
fn boundary() -> VerifiedPsiOptimizationUnit {
    lowered_unit(MULTI_SOURCE_ENTRY_SOURCE, "multi-source entry edge")
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::LoopInvariantScalarMotion,
    expected_rule,
    // The transitive member parameter's `s + s` relocates to the component's
    // unique preheader as one committed roster.
    expected_commits: 1,
    positive,
    boundary,
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

#[test]
fn boundary_workload_declines_and_publishes() {
    assert_boundary_leg(&CASE);
}

#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&CASE);
}

#[test]
fn runs_are_deterministic() {
    assert_determinism_leg(&CASE);
}

#[test]
fn published_unit_is_the_fixed_point() {
    assert_fixed_point_leg(&CASE);
}

#[test]
fn forged_run_axes_fail_publication_replay() {
    assert_corruption_legs(&CASE);
}

#[test]
fn malformed_carriers_fail_admission() {
    assert_malformed_carrier_legs(&CASE);
}

#[test]
fn recorded_decisions_drive_downstream_replay() {
    assert_downstream_replay_leg(&CASE);
}

#[test]
fn stale_and_foreign_context_logs_reject() {
    assert_stale_evidence_leg(&CASE);
}

#[test]
fn unsupported_compositions_decline() {
    assert_composition_leg(&CASE);
}
