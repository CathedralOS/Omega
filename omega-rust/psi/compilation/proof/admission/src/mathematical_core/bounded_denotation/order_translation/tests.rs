use std::collections::BTreeSet;

use super::super::Elaboration;
use crate::{
    Budget, ProofNode, ProofRule, Term, check_certificate, verify_bounded_certificate,
    verify_mathematical_certificate,
};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarType, ValueId,
};

fn integer() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}
fn value(raw: u64) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer(),
        value: ValueId::new(raw).unwrap(),
    }
}
fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=3).map(|raw| (ValueId::new(raw).unwrap(), ScalarType::Integer(integer()))),
    )
    .unwrap()
}
fn translated(
    left: IntegerMathTerm,
    right: IntegerMathTerm,
    addend: IntegerMathTerm,
) -> (Proposition, ProofNode) {
    let premise = Proposition::IntegerMathLessOrEqual(left.clone(), right.clone());
    let conclusion = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(left), Box::new(addend.clone())),
        IntegerMathTerm::Add(Box::new(right), Box::new(addend.clone())),
    );
    (
        premise.clone(),
        ProofNode {
            conclusion,
            rule: ProofRule::IntegerMathOrderTranslation {
                relation: Box::new(ProofNode {
                    conclusion: premise,
                    rule: ProofRule::Assumption { index: 0 },
                }),
                addend,
            },
        },
    )
}

#[test]
fn mathematical_order_translation_derives_from_existing_core_law() {
    // A negative mathematical addend remains signed mathematical subtraction,
    // even though its embedded observations come from unsigned finite values.
    let addend = IntegerMathTerm::Subtract(Box::new(value(3)), Box::new(value(2)));
    let (premise, proof) = translated(value(1), value(2), addend);
    let context = context();
    check_certificate(&context, &proof.conclusion, &[premise.clone()], &[], &proof).unwrap();
    let parameters = BTreeSet::new();
    let premises = [premise];
    let mut elaboration =
        Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(
        elaboration.denotation.rule_axioms.is_empty(),
        "translation must not introduce a rule-instance assumption"
    );
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
    assert!(check_certificate(&context, &proof.conclusion, &[], &[], &proof).is_err());
}

#[test]
fn mathematical_order_translation_retains_closed_numeral_denotation() {
    let literal = |value| IntegerMathTerm::literal(IntegerValue::Signed(value));
    let (premise, proof) = translated(literal(-4), literal(2), literal(-3));
    let context = context();
    check_certificate(&context, &proof.conclusion, &[premise.clone()], &[], &proof).unwrap();
    let parameters = BTreeSet::new();
    let premises = [premise];
    let mut elaboration =
        Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
}

#[test]
fn mathematical_order_translation_rejects_strictness_addend_and_scope_drift() {
    let (premise, proof) = translated(value(1), value(2), value(3));
    let context = context();
    let mut stale = proof.clone();
    let ProofRule::IntegerMathOrderTranslation { addend, .. } = &mut stale.rule else {
        unreachable!()
    };
    *addend = value(2);
    assert!(
        check_certificate(&context, &stale.conclusion, &[premise.clone()], &[], &stale).is_err()
    );

    let strict = Proposition::IntegerMathLessThan(value(1), value(2));
    let mut changed = proof;
    let ProofRule::IntegerMathOrderTranslation { relation, .. } = &mut changed.rule else {
        unreachable!()
    };
    relation.conclusion = strict.clone();
    assert!(check_certificate(&context, &changed.conclusion, &[strict], &[], &changed).is_err());

    // The complete claim changes consistently, but its new captured subject
    // is outside the current proof context and therefore still rejects.
    let (premise, unknown) = translated(value(1), value(2), value(99));
    assert!(check_certificate(&context, &unknown.conclusion, &[premise], &[], &unknown).is_err());
    let wrong_type = IntegerMathTerm::MathValue {
        source_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
        value: ValueId::new(3).unwrap(),
    };
    let (premise, wrong_carrier) = translated(value(1), value(2), wrong_type);
    assert!(
        check_certificate(
            &context,
            &wrong_carrier.conclusion,
            &[premise],
            &[],
            &wrong_carrier
        )
        .is_err()
    );
}

#[test]
fn mathematical_order_substitution_rejoins_only_exact_finite_rings() {
    use semantic_vocabulary::ScalarTerm;
    let scalar =
        |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(integer()));
    let sum = IntegerMathTerm::Add(Box::new(value(2)), Box::new(value(3)));
    let exact = ScalarTerm::exact_integer_add(integer(), scalar(2), scalar(3)).unwrap();
    let equality = Proposition::IntegerMathEqual(value(1), value(3));
    let context = context();
    for strict in [false, true] {
        let relation = if strict {
            Proposition::IntegerMathLessThan(value(1), sum.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(value(1), sum.clone())
        };
        let conclusion = if strict {
            Proposition::LessThan(scalar(3), exact.clone())
        } else {
            Proposition::LessOrEqual(scalar(3), exact.clone())
        };
        let proof = ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: relation.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                equality: Box::new(ProofNode {
                    conclusion: equality.clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
                endpoint: 0,
            },
        };
        let premises = [relation, equality.clone()];
        check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
        let parameters = BTreeSet::new();
        let mut elaboration =
            Elaboration::new(&context, &proof.conclusion, &premises, &[], &parameters).unwrap();
        elaboration.node(&proof).unwrap();
        assert!(elaboration.denotation.rule_axioms.is_empty());
        for changed in [
            ScalarTerm::WrappingIntegerAdd {
                scalar_type: integer(),
                left: Box::new(scalar(2)),
                right: Box::new(scalar(3)),
            },
            ScalarTerm::SaturatingIntegerAdd {
                scalar_type: integer(),
                left: Box::new(scalar(2)),
                right: Box::new(scalar(3)),
            },
        ] {
            let mut wrong = proof.clone();
            wrong.conclusion = if strict {
                Proposition::LessThan(scalar(3), changed)
            } else {
                Proposition::LessOrEqual(scalar(3), changed)
            };
            assert!(crate::lift_exact_integer_order(&wrong.conclusion).is_none());
            assert!(
                check_certificate(&context, &wrong.conclusion, &premises, &[], &wrong).is_err()
            );
        }
    }
}

#[test]
fn mathematical_order_translation_rejects_a_hidden_deep_addend_without_cloning_it() {
    let premise = Proposition::IntegerMathLessOrEqual(value(1), value(2));
    let mut addend = value(3);
    for _ in 0..16_384 {
        addend = IntegerMathTerm::Add(
            Box::new(addend),
            Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(0))),
        );
    }
    let proof = ProofNode {
        // A tiny conclusion has no translated endpoints. The unmatched rule
        // payload must not be traversed or cloned to decide that mismatch.
        conclusion: premise.clone(),
        rule: ProofRule::IntegerMathOrderTranslation {
            relation: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            addend,
        },
    };
    assert_eq!(
        check_certificate(&context(), &proof.conclusion, &[premise], &[], &proof),
        Err(crate::ProofError::IntegerOrderConclusionMismatch)
    );
    // The test owns a deliberately deep Box tree; dismantle it iteratively so
    // recursive Rust destruction does not become this checker's stack bound.
    let ProofRule::IntegerMathOrderTranslation { mut addend, .. } = proof.rule else {
        unreachable!()
    };
    while let IntegerMathTerm::Add(left, _) = addend {
        addend = *left;
    }
}
