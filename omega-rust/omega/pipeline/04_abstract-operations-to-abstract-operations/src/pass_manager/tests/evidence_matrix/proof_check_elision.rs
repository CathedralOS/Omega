//! `Optimization::ProofCheckElision` evidence matrix legs.

use super::super::{verified_exact_add_unit, verified_exact_self_divide_unit};
use super::{
    SelectionMatrix, assert_boundary_leg, assert_corruption_legs, assert_determinism_leg,
    assert_disabled_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_positive_leg,
};
use crate::rules::LiveProofCertifiedIntegerSelfDivideEliminationRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    LiveProofCertifiedIntegerSelfDivideEliminationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::ProofCheckElision,
    expected_rule,
    expected_commits: 1,
    positive: verified_exact_self_divide_unit,
    boundary: verified_exact_add_unit,
    sibling: Optimization::CopyPropagation,
};

#[test]
fn positive_elides_the_certified_self_divide_obligation() {
    assert_positive_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn boundary_declines_a_certified_obligation_no_rule_covers() {
    assert_boundary_leg(&CASE);
}

#[test]
fn disabled_sibling_selection_leaves_the_unit_untouched() {
    assert_disabled_leg(&CASE);
}

#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&CASE);
}

#[test]
fn repeated_runs_are_deterministic() {
    assert_determinism_leg(&CASE);
}

#[test]
fn published_unit_is_a_legal_second_input_fixed_point() {
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
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}
