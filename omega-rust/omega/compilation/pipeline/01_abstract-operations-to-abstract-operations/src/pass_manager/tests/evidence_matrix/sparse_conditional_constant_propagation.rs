//! `Optimization::SparseConditionalConstantPropagation` evidence matrix legs.

use super::super::{VerifiedPsiOptimizationUnit, verified_exact_add_unit};
use super::{
    SelectionMatrix, assert_boundary_leg, assert_composition_leg, assert_corruption_legs,
    assert_determinism_leg, assert_disabled_leg, assert_downstream_replay_leg,
    assert_empty_selection_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_stale_evidence_leg,
};
use crate::ranked_rewrites::loop_invariant_scalar_motion::tests::lowered_unit;
use crate::rules::ExactIntegerAddConstantsRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    ExactIntegerAddConstantsRule::contract().identity()
}

/// A wrapping add with one constant and one parameter operand: one operand
/// short of constant folding, so the pass declines at its admission edge.
const NONCONSTANT_OPERAND_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(x: u32 in Wrapping)
    {
        let y: u32 in Wrapping = x + 3;
        transition { _ -> finish(y) }
        state finish(r: u32 in Wrapping) {}
    }
"#;

fn boundary() -> VerifiedPsiOptimizationUnit {
    lowered_unit(NONCONSTANT_OPERAND_SOURCE, "nonconstant-operand edge")
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::SparseConditionalConstantPropagation,
    expected_rule,
    // The proof-certified `7 + 8` exact add folds to its literal in place.
    expected_commits: 1,
    positive: verified_exact_add_unit,
    boundary,
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
