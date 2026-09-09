use crate::{
    AcceptedProofRule, ProofNode, ProofRule, accept_certificate, check_certificate,
    lift_fixed_integer_relation,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};

fn fixture(
    strict: [bool; 2],
    mathematical: bool,
) -> (PropositionContext, [Proposition; 2], ProofNode) {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let context = PropositionContext::from_value_types(
        (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let order = |left, right, strict| {
        let proposition = if strict {
            Proposition::LessThan(value(left), value(right))
        } else {
            Proposition::LessOrEqual(value(left), value(right))
        };
        if mathematical {
            lift_fixed_integer_relation(&proposition).unwrap()
        } else {
            proposition
        }
    };
    let premises = [order(1, 2, strict[0]), order(2, 3, strict[1])];
    let proof = ProofNode {
        conclusion: order(1, 3, true),
        rule: ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: Box::new(ProofNode {
                conclusion: premises[0].clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            middle_to_right: Box::new(ProofNode {
                conclusion: premises[1].clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    (context, premises, proof)
}

#[test]
fn strict_and_mixed_chains_retain_both_premise_authorities_in_each_domain() {
    for strict in [[true, false], [false, true], [true, true]] {
        for mathematical in [false, true] {
            let (context, premises, proof) = fixture(strict, mathematical);
            let acceptance = accept_certificate(
                &context,
                &proof.conclusion,
                &premises[..1],
                &premises[1..],
                &proof,
            )
            .unwrap();
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::IntegerStrictOrderTransitivity)
            );
            assert_eq!(acceptance.assumptions[0].proposition, premises[0]);
            assert_eq!(acceptance.semantic_axioms[0].proposition, premises[1]);
            assert_eq!(acceptance.assumptions.len(), 1);
            assert_eq!(acceptance.semantic_axioms.len(), 1);
            for (requirements, axioms) in [(&[][..], &premises[1..]), (&premises[..1], &[][..])] {
                assert!(
                    check_certificate(&context, &proof.conclusion, requirements, axioms, &proof)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn nonstrict_chains_and_endpoint_or_domain_drift_reject() {
    for mathematical in [false, true] {
        let (context, premises, proof) = fixture([false, false], mathematical);
        assert!(
            check_certificate(
                &context,
                &proof.conclusion,
                &premises[..1],
                &premises[1..],
                &proof
            )
            .is_err()
        );
        for mutation in [
            "middle",
            "left",
            "right",
            "conclusion kind",
            "mixed domain",
            "false child",
        ] {
            let (context, mut premises, mut proof) = fixture([true, false], mathematical);
            let (_, _, other) = fixture([true, false], !mathematical);
            let ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } = &mut proof.rule
            else {
                unreachable!()
            };
            match mutation {
                "middle" => {
                    premises[1] = premises[0].clone();
                    middle_to_right.conclusion = premises[1].clone();
                }
                "left" => proof.conclusion = premises[1].clone(),
                "right" => proof.conclusion = premises[0].clone(),
                "conclusion kind" => {
                    proof.conclusion = match proof.conclusion {
                        Proposition::LessThan(left, right) => Proposition::LessOrEqual(left, right),
                        Proposition::IntegerMathLessThan(left, right) => {
                            Proposition::IntegerMathLessOrEqual(left, right)
                        }
                        _ => unreachable!(),
                    };
                }
                "mixed domain" => proof.conclusion = other.conclusion,
                "false child" => left_to_middle.rule = ProofRule::SemanticAxiom { index: 0 },
                _ => unreachable!(),
            }
            assert!(
                check_certificate(
                    &context,
                    &proof.conclusion,
                    &premises[..1],
                    &premises[1..],
                    &proof
                )
                .is_err(),
                "{mutation}"
            );
        }
    }
}

#[test]
fn boolean_relations_and_mismatched_integer_carriers_reject() {
    let (context, premises, proof) = fixture([true, false], false);
    let wrong = PropositionContext::from_value_types((1..=4).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            if identity == 2 {
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
            } else {
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
            },
        )
    }))
    .unwrap();
    assert!(
        check_certificate(
            &wrong,
            &proof.conclusion,
            &premises[..1],
            &premises[1..],
            &proof
        )
        .is_err()
    );
    let first = Proposition::LessThan(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true));
    let second = Proposition::LessOrEqual(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true));
    assert!(super::check(&first, &second, &first).is_err());
    check_certificate(
        &context,
        &proof.conclusion,
        &premises[..1],
        &premises[1..],
        &proof,
    )
    .unwrap();
}

#[test]
fn order_composition_preserves_scalar_carriers_without_arithmetic_conversion() {
    for integer in [
        IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
        IntegerType::address(64).unwrap(),
    ] {
        let scalar_type = ScalarType::Integer(integer);
        let context = PropositionContext::from_value_types(
            (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let premises = [
            Proposition::LessThan(value(1), value(2)),
            Proposition::LessOrEqual(value(2), value(3)),
        ];
        let proof = ProofNode {
            conclusion: Proposition::LessThan(value(1), value(3)),
            rule: ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(ProofNode {
                    conclusion: premises[0].clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                middle_to_right: Box::new(ProofNode {
                    conclusion: premises[1].clone(),
                    rule: ProofRule::Assumption { index: 1 },
                }),
            },
        };
        check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
    }
}
