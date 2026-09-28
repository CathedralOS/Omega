//! `match` on a scalar-payload sum subject admits case classifiers and lowers
//! to ordered case-membership selections instead of value-pattern comparison.
use super::check;
use checked_trees::CheckedScalarComputationKind;

fn case_membership_conditions(
    checked: &checked_trees::CheckedTrees,
) -> Vec<CheckedScalarComputationKind> {
    checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .flat_map(|(_, node)| match &node.kind {
            CheckedScalarComputationKind::Select { condition, .. } => {
                vec![
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .nodes
                        .get(*condition)
                        .kind
                        .clone(),
                ]
            }
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn local_sum_subject_dispatches_through_place_membership() {
    let checked = check(
        "data Choice { case Empty; case Full; }
         machine choose() -> i64 {
             let c: Choice = Choice::Empty;
             match c { Choice::Empty -> 1, Choice::Full -> 2, _ -> 0 }
         }",
    )
    .expect("a case-classified match on an immutable local");
    let conditions = case_membership_conditions(&checked);
    assert_eq!(conditions.len(), 2, "{conditions:?}");
    for kind in conditions {
        let CheckedScalarComputationKind::CaseMembership { subject, .. } = kind else {
            panic!("a case-classified arm lowers to a place membership test");
        };
        assert!(matches!(
            subject,
            checked_trees::CheckedScalarComputationStructuralArgument::Place(_)
        ));
    }
}

#[test]
fn self_field_sum_subject_dispatches_through_parameter_membership() {
    let checked = check(
        "data Choice { case Empty; case Full; }
         data Holder { res: Choice }
         machine Holder::classify(&mut self) -> i64 {
             match self.res { Choice::Empty -> 1, Choice::Full -> 2, _ -> 0 }
         }",
    )
    .expect("a case-classified match on an attached-data field");
    let conditions = case_membership_conditions(&checked);
    assert_eq!(conditions.len(), 2, "{conditions:?}");
    for kind in conditions {
        let CheckedScalarComputationKind::CaseMembership { subject, .. } = kind else {
            panic!("a parameter-field subject lowers to structural case membership");
        };
        assert!(matches!(
            subject,
            checked_trees::CheckedScalarComputationStructuralArgument::Place(argument)
                if matches!(
                    argument.source,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
                ) && !argument.path.is_empty()
        ));
    }
}

#[test]
fn case_dispatch_still_requires_coverage() {
    let diagnostics = check(
        "data Choice { case Empty; case Full; }
         machine choose() -> i64 {
             let c: Choice = Choice::Empty;
             match c { Choice::Empty -> 1 }
         }",
    )
    .expect_err("a single case does not cover the sum");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cover")
                || diagnostic.message.contains("exhaustive")),
        "{diagnostics:#?}"
    );
}

#[test]
fn case_dispatch_rejects_a_foreign_case_classifier() {
    let diagnostics = check(
        "data Choice { case Empty; case Full; }
         data Other { case Far; }
         machine choose() -> i64 {
             let c: Choice = Choice::Empty;
             match c { Other::Far -> 1, _ -> 0 }
         }",
    )
    .expect_err("a case of a different sum cannot classify the subject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("scalar")
                || diagnostic.message.contains("not supported")),
        "{diagnostics:#?}"
    );
}

#[test]
fn case_dispatch_rejects_a_payload_case_classifier() {
    let diagnostics = check(
        "data Choice { case Empty; case Some(value: u32); }
         machine choose() -> i64 {
             let c: Choice = Choice::Empty;
             match c { Choice::Some -> 7, _ -> 0 }
         }",
    )
    .expect_err("a bare payload-case name is not a value");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("carries a payload")),
        "{diagnostics:#?}"
    );
}

#[test]
fn case_dispatch_rejects_a_structural_value_pattern() {
    let diagnostics = check(
        "data Choice { case Empty; case Full; }
         machine choose() -> i64 {
             let c: Choice = Choice::Empty;
             let other: Choice = Choice::Full;
             match c { other -> 1, _ -> 0 }
         }",
    )
    .expect_err("a structural value is not a case classifier");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("scalar")
                || diagnostic.message.contains("not supported")),
        "{diagnostics:#?}"
    );
}

#[test]
fn case_dispatch_rejects_structural_arm_results() {
    let diagnostics = check(
        "data Choice { case Empty; case Full; }
         machine choose() -> Choice {
             let c: Choice = Choice::Empty;
             match c { Choice::Empty -> c, _ -> c }
         }",
    )
    .expect_err("discriminant dispatch emits scalar selections only");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("scalar")
                || diagnostic.message.contains("not supported")),
        "{diagnostics:#?}"
    );
}
