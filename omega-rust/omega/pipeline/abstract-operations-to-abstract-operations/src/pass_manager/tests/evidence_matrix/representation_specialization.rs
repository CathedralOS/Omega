//! `Optimization::RepresentationSpecialization` evidence matrix legs.

use super::super::{
    verified_cyclic_field_value_unit, verified_cyclic_membership_unit,
    verified_field_value_decline_unit, verified_field_value_unit, verified_membership_decline_unit,
    verified_representation_specialization_unit,
};
use super::{
    SelectionMatrix, VerifiedPsiOptimizationSession, assert_boundary_leg, assert_corruption_legs,
    assert_determinism_leg, assert_disabled_leg, assert_fixed_point_leg, assert_full_entrance_leg,
    assert_malformed_carrier_legs, assert_measured_budget_leg, assert_negative_leg,
    assert_positive_leg, budget, run_psi_pipeline, selections_of,
};
use crate::rules::{CaseMembershipSpecializationRule, FieldValueSpecializationRule};
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    CaseMembershipSpecializationRule::contract().identity()
}

fn expected_field_rule() -> OptimizationRuleIdentity {
    FieldValueSpecializationRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::RepresentationSpecialization,
    expected_rule,
    // One candidate covers the established place: its two memberships fold
    // together — `c in Choice::Some` to `true` and `c in Choice::Empty` to
    // `false` — under the `EstablishScalarCase` producer basis, so the pass
    // commits exactly once.
    expected_commits: 1,
    positive: verified_representation_specialization_unit,
    boundary: verified_membership_decline_unit,
    sibling: Optimization::DeadPureScalarElimination,
};

#[test]
fn positive_folds_the_proven_membership_observations() {
    assert_positive_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}

#[test]
fn boundary_declines_an_unproven_parameter_membership() {
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

/// Frozen territory: a proven membership inside a machine holding an
/// authenticated cyclic component is declined by the whole selected pass and
/// the machine stays byte-exact.
#[test]
fn cyclic_machine_membership_stays_frozen() {
    let unit = verified_cyclic_membership_unit();
    let session =
        VerifiedPsiOptimizationSession::new(unit.clone()).expect("cyclic fixture re-admits");
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    let membership_count = unit
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                abstract_operations::AbstractOperation::StructuralCaseMembership { .. }
            )
        })
        .count();
    assert!(
        membership_count > 0,
        "the fixture must actually contain a membership to freeze"
    );
    let input_identity = unit.unit().identity;
    let run = run_psi_pipeline(unit, &selections_of(CASE.selection), budget(64)).unwrap();
    assert!(
        run.commits().is_empty(),
        "no membership inside frozen territory specializes"
    );
    assert_eq!(run.session().unit().identity, input_identity);
}

const FIELD: SelectionMatrix = SelectionMatrix {
    selection: Optimization::RepresentationSpecialization,
    expected_rule: expected_field_rule,
    // One candidate covers the established place: its two scalar field reads
    // fold together — `p.x` to `37` and `p.flag` to `true` — under the
    // `EstablishRecord` producer basis, so the pass commits exactly once and
    // no membership is present to draw a second commit.
    expected_commits: 1,
    positive: verified_field_value_unit,
    boundary: verified_field_value_decline_unit,
    sibling: Optimization::DeadPureScalarElimination,
};

#[test]
fn positive_folds_the_proven_field_value_observations() {
    assert_positive_leg(&FIELD);
}

#[test]
fn negative_leaves_an_empty_unit_untouched_for_fields() {
    assert_negative_leg(&FIELD);
}

#[test]
fn boundary_declines_an_unproven_parameter_field_read() {
    assert_boundary_leg(&FIELD);
}

#[test]
fn disabled_sibling_selection_leaves_the_field_unit_untouched() {
    assert_disabled_leg(&FIELD);
}

#[test]
fn measured_budget_admits_exact_field_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&FIELD);
}

#[test]
fn repeated_field_value_runs_are_deterministic() {
    assert_determinism_leg(&FIELD);
}

#[test]
fn published_field_value_unit_is_a_legal_second_input_fixed_point() {
    assert_fixed_point_leg(&FIELD);
}

#[test]
fn forged_field_value_run_axes_fail_publication_replay() {
    assert_corruption_legs(&FIELD);
}

#[test]
fn malformed_field_value_carriers_fail_admission() {
    assert_malformed_carrier_legs(&FIELD);
}

#[test]
fn full_entrance_optimizes_and_publishes_field_values() {
    assert_full_entrance_leg(&FIELD);
}

/// Frozen territory for field values: a bound-proven field read inside a
/// machine holding an authenticated cyclic component is declined by the whole
/// selected pass and the machine stays byte-exact.
#[test]
fn cyclic_machine_field_read_stays_frozen() {
    let unit = verified_cyclic_field_value_unit();
    let session =
        VerifiedPsiOptimizationSession::new(unit.clone()).expect("cyclic fixture re-admits");
    assert!(
        !session.cycle_components().components().is_empty(),
        "the fixture carries an authenticated cyclic component"
    );
    let field_read_count = unit
        .unit()
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.nodes)
        .filter(|node| {
            matches!(
                node.operation,
                abstract_operations::AbstractOperation::IntegerStructuralField { .. }
                    | abstract_operations::AbstractOperation::BooleanStructuralField { .. }
            )
        })
        .count();
    assert!(
        field_read_count > 0,
        "the fixture must actually contain a field read to freeze"
    );
    let input_identity = unit.unit().identity;
    let run = run_psi_pipeline(unit, &selections_of(FIELD.selection), budget(64)).unwrap();
    assert!(
        run.commits().is_empty(),
        "no field read inside frozen territory specializes"
    );
    assert_eq!(run.session().unit().identity, input_identity);
}
