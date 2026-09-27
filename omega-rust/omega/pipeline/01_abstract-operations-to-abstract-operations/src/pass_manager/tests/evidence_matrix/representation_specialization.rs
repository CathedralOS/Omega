//! `Optimization::RepresentationSpecialization` evidence matrix legs.

use super::super::{
    verified_cyclic_field_value_unit, verified_cyclic_membership_unit, verified_field_value_unit,
    verified_forwarded_field_value_unit, verified_representation_specialization_unit,
};
use super::{
    SelectionMatrix, VerifiedPsiOptimizationSession, assert_corruption_legs, assert_disabled_leg,
    assert_empty_selection_leg, assert_full_entrance_leg, assert_negative_leg, budget,
    publish_optimization_run, run_psi_pipeline, selections_of,
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
    sibling: Optimization::DeadPureScalarElimination,
};

#[test]
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}

#[test]
fn disabled_sibling_selection_leaves_the_membership_unit_untouched() {
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
fn forged_run_axes_fail_publication_replay() {
    assert_corruption_legs(&CASE);
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
    sibling: Optimization::DeadPureScalarElimination,
};

#[test]
fn full_entrance_specializes_field_values() {
    assert_full_entrance_leg(&FIELD);
}

#[test]
fn disabled_sibling_selection_leaves_the_field_unit_untouched() {
    assert_disabled_leg(&FIELD);
}

/// The forwarded-initializer leg: `p.x`'s `EstablishRecord` initializer is
/// the nonconstant `x` machine parameter, so the read substitutes the
/// parameter at its compare use and retires while `p.flag` folds — one
/// mixed-resolution candidate, one commit.
const FORWARDED_FIELD: SelectionMatrix = SelectionMatrix {
    selection: Optimization::RepresentationSpecialization,
    expected_rule: expected_field_rule,
    expected_commits: 1,
    positive: verified_forwarded_field_value_unit,
    sibling: Optimization::DeadPureScalarElimination,
};

/// The forwarding commit carries exactly the candidate's `result →
/// initializer` substitution, and the published unit holds no remaining
/// structural-field observation for the established place.
#[test]
fn forwarded_field_value_commit_substitutes_and_retires_the_read() {
    let unit = verified_forwarded_field_value_unit();
    let input_identity = unit.unit().identity;
    let run =
        run_psi_pipeline(unit, &selections_of(FORWARDED_FIELD.selection), budget(64)).unwrap();
    let [commit] = run.commits() else {
        panic!("one commit")
    };
    assert_eq!(commit.rule, expected_field_rule());
    let [substitution] = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .substitutions()
    else {
        panic!("one scalar substitution")
    };
    let optimization_unit::PsiRewritePatch::SpecializeFieldValue(patch) = commit
        .declaration
        .rewrite()
        .expect("rewrite declaration")
        .patch()
    else {
        panic!("a field-value patch")
    };
    assert!(patch.reads.iter().any(|row| {
        matches!(
            &row.resolution,
            optimization_unit::FieldValueResolution::Forward(forwarded)
                if row.result == substitution.from && forwarded.initializer == substitution.to
        )
    }));
    let plan = publish_optimization_run(run).unwrap();
    assert_ne!(plan.unit().identity, input_identity);
    let remaining = plan
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
    assert_eq!(remaining, 0, "both proven reads resolve");
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
