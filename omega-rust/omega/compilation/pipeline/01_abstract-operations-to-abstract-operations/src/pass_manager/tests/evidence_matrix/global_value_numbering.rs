//! `Optimization::GlobalValueNumbering` evidence matrix legs.

use super::super::{VerifiedPsiOptimizationUnit, verified_compatible_policy_cse_unit};
use super::{
    SelectionMatrix, assert_boundary_leg, assert_composition_leg, assert_corruption_legs,
    assert_determinism_leg, assert_disabled_leg, assert_downstream_replay_leg,
    assert_empty_selection_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_stale_evidence_leg,
};
use crate::ranked_rewrites::loop_invariant_scalar_motion::tests::lowered_unit;
use crate::rules::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity()
}

/// Two same-kind wrapping adds over distinct operand pairs: the numbering
/// edge without a repeated value, so the pass declines.
const DISTINCT_OPERANDS_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(x: u32 in Wrapping, y: u32 in Wrapping)
    {
        let a: u32 in Wrapping = x + y;
        let b: u32 in Wrapping = y + y;
        let c: u32 in Wrapping = a + b;
        transition { _ -> finish(c) }
        state finish(r: u32 in Wrapping) {}
    }
"#;

fn boundary() -> VerifiedPsiOptimizationUnit {
    lowered_unit(DISTINCT_OPERANDS_SOURCE, "distinct-operand edge")
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::GlobalValueNumbering,
    expected_rule,
    // The proof-certified `right + left` exact add repeats the earlier
    // `left + right` wrapping add under the compatible-policy lattice: one
    // substitution, one commit.
    expected_commits: 1,
    positive: verified_compatible_policy_cse_unit,
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
