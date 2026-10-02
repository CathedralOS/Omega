use std::collections::BTreeSet;

use super::Elaboration;
use crate::{
    AcceptedProofRule, Budget, ProofNode, ProofRule, Term, accept_certificate, check_certificate,
    verify_bounded_certificate, verify_mathematical_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

fn value(carrier: IntegerType, identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(carrier),
    )
}
fn context(carrier: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(carrier),
        )
    }))
    .unwrap()
}
fn proof(left: ScalarTerm, right: ScalarTerm) -> (Vec<Proposition>, ProofNode) {
    let forward = Proposition::LessOrEqual(left.clone(), right.clone());
    let backward = Proposition::LessOrEqual(right.clone(), left.clone());
    let result = ProofNode {
        conclusion: Proposition::Equal(left, right),
        rule: ProofRule::IntegerOrderAntisymmetry {
            forward: Box::new(ProofNode {
                conclusion: forward.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            backward: Box::new(ProofNode {
                conclusion: backward.clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    (vec![forward, backward], result)
}

#[test]
fn integer_order_antisymmetry_certifies_both_orders_through_existing_core_law() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let carrier = IntegerType::new(sign, 64).unwrap();
        let context = context(carrier);
        let (premises, proof) = proof(value(carrier, 1), value(carrier, 2));
        let accepted =
            accept_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
        assert!(
            accepted
                .rules
                .contains(&AcceptedProofRule::IntegerOrderAntisymmetry)
        );
        let parameters = BTreeSet::new();
        let mut elaboration =
            Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
        elaboration.node(&proof).unwrap();
        assert!(elaboration.denotation.rule_axioms.is_empty());
        let mut denoted = verify_bounded_certificate(
            &context,
            &proof.conclusion,
            &premises,
            &[],
            &proof,
            &mut Budget::default(),
        )
        .unwrap();
        denoted.certificate.term = denoted.arena.insert(Term::TwoZero);
        assert!(
            verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut Budget::default()
            )
            .is_err()
        );
        assert!(
            check_certificate(&context, &proof.conclusion, &premises[..1], &[], &proof).is_err()
        );
    }
}

#[test]
fn integer_order_antisymmetry_rejects_strictness_endpoint_and_scope_drift() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let (original, proof) = proof(value(carrier, 1), value(carrier, 2));
    for replacement in [
        Proposition::LessOrEqual(value(carrier, 1), value(carrier, 2)),
        Proposition::LessOrEqual(value(carrier, 2), value(carrier, 3)),
        Proposition::LessThan(value(carrier, 2), value(carrier, 1)),
        Proposition::LessOrEqual(value(carrier, 2), value(carrier, 99)),
    ] {
        let mut changed = proof.clone();
        let ProofRule::IntegerOrderAntisymmetry { backward, .. } = &mut changed.rule else {
            unreachable!()
        };
        backward.conclusion = replacement.clone();
        let premises = [original[0].clone(), replacement];
        assert!(
            check_certificate(&context, &changed.conclusion, &premises, &[], &changed).is_err()
        );
    }
    let mut changed = proof.clone();
    changed.conclusion = Proposition::Equal(value(carrier, 1), value(carrier, 3));
    assert!(check_certificate(&context, &changed.conclusion, &original, &[], &changed).is_err());
    let mut changed = proof;
    let ProofRule::IntegerOrderAntisymmetry { forward, backward } = &mut changed.rule else {
        unreachable!()
    };
    std::mem::swap(forward, backward);
    assert!(check_certificate(&context, &changed.conclusion, &original, &[], &changed).is_err());
}

#[test]
fn integer_order_antisymmetry_does_not_relabel_carriers_or_arithmetic_policy() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    for other in [
        IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
        IntegerType::new(IntegerSign::Signed, 64).unwrap(),
        IntegerType::address(64).unwrap(),
    ] {
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(carrier)),
            (ValueId::new(2).unwrap(), ScalarType::Integer(other)),
        ])
        .unwrap();
        let (premises, proof) = proof(value(carrier, 1), value(other, 2));
        assert!(check_certificate(&context, &proof.conclusion, &premises, &[], &proof).is_err());
    }
    let wrapped = ScalarTerm::WrappingIntegerAdd {
        scalar_type: carrier,
        left: Box::new(value(carrier, 1)),
        right: Box::new(value(carrier, 2)),
    };
    let (premises, mut proof) = proof(wrapped.clone(), value(carrier, 3));
    // Antisymmetry works on the actual wrapped value when both orders hold.
    check_certificate(&context(carrier), &proof.conclusion, &premises, &[], &proof).unwrap();
    let ScalarTerm::WrappingIntegerAdd {
        scalar_type,
        left,
        right,
    } = wrapped
    else {
        unreachable!()
    };
    proof.conclusion = Proposition::Equal(
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        },
        value(carrier, 3),
    );
    assert!(
        check_certificate(&context(carrier), &proof.conclusion, &premises, &[], &proof).is_err()
    );
}

#[test]
fn integer_order_antisymmetry_orients_literal_and_reversed_value_equalities() {
    use semantic_vocabulary::IntegerValue;
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let zero = ScalarTerm::integer(carrier, IntegerValue::Unsigned(0)).unwrap();
    for (left, right) in [
        (value(carrier, 1), zero),
        (value(carrier, 2), value(carrier, 1)),
    ] {
        let (premises, proof) = proof(left, right);
        check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
        let parameters = BTreeSet::new();
        let mut elaboration =
            Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
        elaboration.node(&proof).unwrap();
        assert!(elaboration.denotation.rule_axioms.is_empty());
    }
}
