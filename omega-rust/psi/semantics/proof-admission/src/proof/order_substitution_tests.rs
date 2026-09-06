use super::*;
use crate::PrimitiveJudgment;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

fn fixture(
    strict: bool,
    mathematical: bool,
    reverse: bool,
) -> (PropositionContext, Vec<Proposition>, ProofNode) {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let literal =
        |integer| ScalarTerm::integer(integer_type, IntegerValue::Signed(integer)).unwrap();
    let project = |proposition: Proposition| {
        if mathematical {
            lift_fixed_integer_relation(&proposition).unwrap()
        } else {
            proposition
        }
    };
    let order = |left, right| {
        project(if strict {
            Proposition::LessThan(left, right)
        } else {
            Proposition::LessOrEqual(left, right)
        })
    };
    let mut proof = ProofNode {
        conclusion: order(literal(1), literal(2)),
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    };
    let mut axioms = Vec::new();
    for endpoint in 0..2 {
        let scalar = value(endpoint as u64 + 1);
        let constant = literal(endpoint as i128 + 1);
        let equality = project(if reverse {
            Proposition::Equal(constant, scalar.clone())
        } else {
            Proposition::Equal(scalar.clone(), constant)
        });
        axioms.push(equality.clone());
        let conclusion = if endpoint == 0 {
            order(scalar, literal(2))
        } else {
            order(value(1), scalar)
        };
        proof = ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(proof),
                equality: Box::new(ProofNode {
                    conclusion: equality,
                    rule: ProofRule::SemanticAxiom { index: endpoint },
                }),
                endpoint,
            },
        };
    }
    (context, axioms, proof)
}

#[test]
fn strict_and_nonstrict_orders_replay_both_endpoints_and_equality_directions() {
    for strict in [false, true] {
        for mathematical in [false, true] {
            for reverse in [false, true] {
                let (context, axioms, proof) = fixture(strict, mathematical, reverse);
                check_certificate(&context, &proof.conclusion, &[], &axioms, &proof).unwrap();
                assert!(
                    check_certificate(&context, &proof.conclusion, &[], &axioms[..1], &proof)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn substitution_cannot_change_order_strictness_in_either_direction() {
    for strict in [false, true] {
        for mathematical in [false, true] {
            let (context, axioms, mut proof) = fixture(strict, mathematical, false);
            proof.conclusion = match proof.conclusion {
                Proposition::LessThan(left, right) => Proposition::LessOrEqual(left, right),
                Proposition::LessOrEqual(left, right) => Proposition::LessThan(left, right),
                Proposition::IntegerMathLessThan(left, right) => {
                    Proposition::IntegerMathLessOrEqual(left, right)
                }
                Proposition::IntegerMathLessOrEqual(left, right) => {
                    Proposition::IntegerMathLessThan(left, right)
                }
                _ => unreachable!(),
            };
            assert_eq!(
                check_certificate(&context, &proof.conclusion, &[], &axioms, &proof),
                Err(ProofError::IntegerOrderConclusionMismatch)
            );
        }
    }
}

#[test]
fn substitution_rejects_endpoint_equality_and_carrier_drift() {
    let (context, axioms, original) = fixture(true, false, false);
    for mutation in 0..4 {
        let mut proof = original.clone();
        let ProofRule::IntegerOrderSubstitution {
            equality, endpoint, ..
        } = &mut proof.rule
        else {
            unreachable!()
        };
        match mutation {
            0 => *endpoint = 2,
            1 => *endpoint = 0,
            2 => equality.rule = ProofRule::SemanticAxiom { index: 0 },
            _ => {
                let Proposition::LessThan(left, right) = &mut proof.conclusion else {
                    unreachable!()
                };
                *left = right.clone();
            }
        }
        assert!(
            check_certificate(&context, &proof.conclusion, &[], &axioms, &proof).is_err(),
            "mutation {mutation}"
        );
    }
    let wrong_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Boolean),
        (
            ValueId::new(2).unwrap(),
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
        ),
    ])
    .unwrap();
    assert!(
        check_certificate(
            &wrong_context,
            &original.conclusion,
            &[],
            &axioms,
            &original
        )
        .is_err()
    );
}

#[test]
fn strict_substitution_preserves_existing_fixed_to_math_conclusion_projection() {
    let (context, axioms, mut proof) = fixture(true, false, false);
    proof.conclusion = lift_fixed_integer_relation(&proof.conclusion).unwrap();
    check_certificate(&context, &proof.conclusion, &[], &axioms, &proof).unwrap();
}
