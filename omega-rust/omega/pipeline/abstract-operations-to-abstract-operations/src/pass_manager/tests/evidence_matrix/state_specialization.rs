//! `Optimization::StateSpecialization` evidence matrix legs.

use super::super::{verified_dispatch_all_constant_unit, verified_dispatch_specialization_unit};
use super::{
    SelectionMatrix, assert_boundary_leg, assert_corruption_legs, assert_determinism_leg,
    assert_disabled_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_positive_leg,
};
use crate::rules::StateArgumentSpecializationRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    StateArgumentSpecializationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::StateSpecialization,
    expected_rule,
    // Only the constant-supplied incoming edge fuses; the parameter-supplied
    // edge keeps the dispatch reachable.
    expected_commits: 1,
    positive: verified_dispatch_specialization_unit,
    boundary: verified_dispatch_all_constant_unit,
    sibling: Optimization::GlobalValueNumbering,
};

#[test]
fn positive_fuses_the_constant_supplied_dispatch_edge() {
    assert_positive_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn boundary_declines_an_all_constant_dispatch() {
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
