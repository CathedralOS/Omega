use super::*;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType};

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn declaration(raw: u64) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(raw).unwrap(),
        scalar_type: ScalarType::Integer(integer()),
        qualifications: Default::default(),
    }
}

fn value(raw: u64) -> ScalarTerm {
    ScalarTerm::value(declaration(raw).id, declaration(raw).scalar_type)
}

fn literal(raw: u128) -> ScalarTerm {
    ScalarTerm::integer(integer(), IntegerValue::Unsigned(raw)).unwrap()
}

fn expression(accumulator: u64, remaining: u64) -> ScalarTerm {
    let twice_accumulator =
        ScalarTerm::wrapping_integer_multiply(integer(), value(accumulator), literal(2)).unwrap();
    let successor =
        ScalarTerm::wrapping_integer_add(integer(), value(remaining), literal(1)).unwrap();
    let remaining_sum =
        ScalarTerm::wrapping_integer_multiply(integer(), value(remaining), successor).unwrap();
    ScalarTerm::wrapping_integer_add(integer(), twice_accumulator, remaining_sum).unwrap()
}

fn equality(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    if left < right {
        Proposition::Equal(left, right)
    } else {
        Proposition::Equal(right, left)
    }
}

#[test]
fn scalar_conservation_preserves_wrapping_policy_and_simultaneous_subjects() {
    let parameters = [declaration(1), declaration(2)];
    // Replacement 2 is itself an invocation identity: it must not become 3.
    let bindings = BTreeMap::from([
        (declaration(1).id, declaration(2)),
        (declaration(2).id, declaration(3)),
    ]);
    let original = expression(1, 2);
    let expected = expression(2, 3);
    let mut candidates = Vec::new();
    assert_eq!(
        discover(
            &original,
            &parameters,
            &bindings,
            &mut 4096,
            &mut candidates,
            0
        ),
        Some(expected.clone())
    );
    assert!(candidates.contains(&equality(original.clone(), expected)));
    assert!(!candidates.contains(&equality(original, expression(3, 3))));
}

#[test]
fn scalar_conservation_rejects_foreign_subjects_carriers_and_storage_snapshots() {
    let parameters = [declaration(1), declaration(2)];
    let bindings = BTreeMap::from([
        (declaration(1).id, declaration(3)),
        (declaration(2).id, declaration(4)),
    ]);
    let mut wrong_carrier = declaration(3);
    wrong_carrier.scalar_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
    let incorrect = BTreeMap::from([(declaration(1).id, wrong_carrier)]);
    for (term, bindings) in [
        (value(9), &bindings),
        (value(1), &incorrect),
        (
            ScalarTerm::view_extent(PlaceId::new(1).unwrap(), Vec::new(), integer()),
            &bindings,
        ),
    ] {
        let mut candidates = Vec::new();
        assert!(discover(&term, &parameters, bindings, &mut 4096, &mut candidates, 0).is_none());
        assert!(candidates.is_empty());
    }
    // A result-dependent parent cannot hide an independent formal-only child.
    let scoped = expression(1, 2);
    let term = ScalarTerm::wrapping_integer_add(integer(), value(9), scoped.clone()).unwrap();
    let mut candidates = Vec::new();
    assert!(discover(&term, &parameters, &bindings, &mut 4096, &mut candidates, 0).is_none());
    assert!(candidates.contains(&equality(scoped, expression(3, 4))));
    assert!(
        !candidates
            .iter()
            .any(|candidate| candidate.any_value_id(|id| id == declaration(9).id))
    );
}

#[test]
fn scalar_conservation_keeps_nonwrapping_constructors_and_obeys_shared_budget() {
    let parameters = [declaration(1)];
    let bindings = BTreeMap::from([(declaration(1).id, declaration(2))]);
    let original = ScalarTerm::saturating_integer_add(integer(), value(1), literal(1)).unwrap();
    let expected = ScalarTerm::saturating_integer_add(integer(), value(2), literal(1)).unwrap();
    let mut candidates = Vec::new();
    assert_eq!(
        discover(
            &original,
            &parameters,
            &bindings,
            &mut 4096,
            &mut candidates,
            0
        ),
        Some(expected)
    );
    let mut remaining = 0;
    let mut declined = Vec::new();
    assert!(
        discover(
            &original,
            &parameters,
            &bindings,
            &mut remaining,
            &mut declined,
            0
        )
        .is_none()
    );
    assert!(declined.is_empty());
    assert_eq!(remaining, 0);
}

#[test]
fn scalar_conservation_backedge_replays_wrapping_updates_and_retained_equality() {
    check_wrapping_backedge(false);
}

#[test]
fn scalar_conservation_backedge_prioritizes_the_goal_in_a_dense_equality_roster() {
    check_wrapping_backedge(true);
}

fn check_wrapping_backedge(dense: bool) {
    use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;
    use semantic_vocabulary::PropositionContext;
    use std::collections::BTreeSet;

    let context = PropositionContext::from_value_types(
        (1..=24).map(|raw| (declaration(raw).id, declaration(raw).scalar_type)),
    )
    .unwrap();
    let premise = equality(expression(2, 1), expression(4, 3));
    let goal = equality(expression(2, 1), expression(6, 5));
    let mut equations = vec![
        equality(
            value(5),
            ScalarTerm::wrapping_integer_subtract(integer(), value(3), literal(1)).unwrap(),
        ),
        equality(
            value(6),
            ScalarTerm::wrapping_integer_add(integer(), value(4), value(3)).unwrap(),
        ),
    ];
    if dense {
        // These unrelated exact compounds precede wrapping constructors in
        // term order and exhaust an indiscriminate 96-pair prefix. The needed
        // bridge still joins only the two goal components, under the same
        // finite search limits.
        equations.extend((7..=24).map(|raw| {
            equality(
                value(raw),
                ScalarTerm::exact_integer_add(integer(), value(3), literal(raw.into())).unwrap(),
            )
        }));
    }
    let assumptions = [premise];
    let proof = produce_checked_canonical_integer_proof(
        &context,
        &goal,
        &assumptions,
        &equations,
        &BTreeSet::new(),
        &mut crate::proofs::ProofSearchDeclines::default(),
    )
    .expect("the current header conservation survives the exact wrapping update definitions");
    proof_admission::check_certificate(&context, &goal, &assumptions, &equations, &proof).unwrap();
    assert!(proof_admission::check_certificate(&context, &goal, &[], &equations, &proof).is_err());
    assert!(
        produce_checked_canonical_integer_proof(
            &context,
            &goal,
            &[],
            &equations,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
        .is_none()
    );
    let mut wrong = equations.clone();
    wrong[1] = equality(value(6), value(4));
    assert!(
        produce_checked_canonical_integer_proof(
            &context,
            &goal,
            &assumptions,
            &wrong,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
        .is_none()
    );
    assert!(
        proof_admission::check_certificate(&context, &goal, &assumptions, &wrong, &proof).is_err()
    );
}
