//! `Optimization::DeadPureScalarElimination` evidence matrix legs.

use super::super::{
    OptimizationSelections, budget, verified_dead_literals_unit, verified_half_dead_literals_unit,
    verified_parameter_add_unit,
};
use super::{
    SelectionMatrix, assert_boundary_leg, assert_corruption_legs, assert_determinism_leg,
    assert_disabled_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_positive_leg,
};
use crate::DeadScalarLiteralEliminationRule;
use crate::run_psi_pipeline;
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
    boundary: verified_parameter_add_unit,
    sibling: Optimization::CopyPropagation,
};

#[test]
fn positive_eliminates_unused_scalar_literals() {
    assert_positive_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn boundary_declines_scalar_work_whose_results_all_live() {
    assert_boundary_leg(&CASE);
}

/// The use boundary inside one block: the dead literal commits, the returned
/// literal survives.
#[test]
fn boundary_commits_only_the_dead_literal_beside_a_live_one() {
    let run = run_psi_pipeline(
        verified_half_dead_literals_unit(),
        &OptimizationSelections::new([Optimization::DeadPureScalarElimination]).unwrap(),
        budget(64),
    )
    .unwrap();
    assert_eq!(run.commits().len(), 1);
    assert_eq!(run.commits()[0].rule, expected_rule());
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
