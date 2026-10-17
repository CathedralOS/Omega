use crate::common::*;
use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, accept_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};

fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=32).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap()
}

fn equal(left: ScalarTerm, right: ScalarTerm) -> ScalarTerm {
    ScalarTerm::boolean_equal(left, right).unwrap()
}

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

#[test]
fn byte_content_equations_require_the_exact_observation_and_evidence() {
    use semantic_vocabulary::{ByteSequencePredicate, PlaceId, StructuralPlaceKind};
    let roots = [PlaceId::new(1).unwrap(), PlaceId::new(2).unwrap()];
    let context = PropositionContext::from_value_types_and_places(
        [],
        roots.into_iter().enumerate().map(|(position, root)| {
            (
                root,
                StructuralPlaceKind::Parameter {
                    position: position as u32,
                },
            )
        }),
    )
    .unwrap();
    let observation = |root| ScalarTerm::ByteSequencePredicate {
        predicate: ByteSequencePredicate::AsciiOnly,
        root,
        path: Vec::new(),
    };
    let goal = Proposition::Equal(observation(roots[0]), ScalarTerm::Boolean(true));
    let equations = vec![goal.clone()];
    let proof = transport(goal.clone(), axiom_proofs(&equations));
    assert!(accept_certificate(&context, &goal, &[], &equations, &proof).is_ok());
    assert!(accept_certificate(&context, &goal, &[], &[], &proof).is_err());
    for equation in [
        Proposition::Equal(observation(roots[1]), ScalarTerm::Boolean(true)),
        Proposition::Equal(observation(roots[0]), ScalarTerm::Boolean(false)),
    ] {
        let equations = vec![equation];
        let proof = transport(goal.clone(), axiom_proofs(&equations));
        assert!(accept_certificate(&context, &goal, &[], &equations, &proof).is_err());
    }
}

fn axiom_proofs(equations: &[Proposition]) -> Vec<ProofNode> {
    equations
        .iter()
        .enumerate()
        .map(|(index, conclusion)| ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::SemanticAxiom { index },
        })
        .collect()
}

fn transport(goal: Proposition, equalities: Vec<ProofNode>) -> ProofNode {
    ProofNode {
        conclusion: goal,
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(truth()),
            equalities,
        },
    }
}

fn fixture() -> (Proposition, Vec<Proposition>, ProofNode) {
    let equations = vec![
        Proposition::Truth,
        Proposition::Equal(value(3), equal(value(1), value(2))),
        Proposition::Equal(value(6), equal(value(4), value(5))),
        Proposition::Equal(value(7), equal(value(3), value(6))),
    ];
    let goal = Proposition::Equal(
        value(7),
        equal(equal(value(1), value(2)), equal(value(4), value(5))),
    );
    let proof = transport(
        goal.clone(),
        axiom_proofs(&equations).into_iter().skip(1).collect(),
    );
    (goal, equations, proof)
}

#[test]
fn ambient_equations_cannot_replace_missing_or_unproved_children() {
    let (goal, equations, original) = fixture();
    for mutation in 0..5 {
        let mut proof = original.clone();
        let ProofRule::ValueEqualityTransport {
            premise,
            equalities,
        } = &mut proof.rule
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                equalities.remove(0);
            }
            1 => equalities.clear(),
            2 => equalities[0].rule = ProofRule::SemanticAxiom { index: 2 },
            3 => equalities[0].rule = ProofRule::Primitive(PrimitiveJudgment::Truth),
            4 => premise.conclusion = Proposition::Falsehood,
            _ => unreachable!(),
        }
        assert!(
            accept_certificate(&context(), &goal, &[], &equations, &proof).is_err(),
            "mutation {mutation}"
        );
    }
    let mut changed = equations.clone();
    changed[1] = Proposition::Equal(value(3), equal(value(1), value(4)));
    assert!(accept_certificate(&context(), &goal, &[], &changed, &original).is_err());
}

#[test]
fn guarded_or_unoriented_equations_are_not_definitions() {
    let equality = Proposition::Equal(value(2), ScalarTerm::boolean_not(value(1)).unwrap());
    let goal = Proposition::Equal(
        ScalarTerm::boolean_not(value(2)).unwrap(),
        ScalarTerm::boolean_not(ScalarTerm::boolean_not(value(1)).unwrap()).unwrap(),
    );
    let Proposition::Equal(left, right) = &equality else {
        unreachable!()
    };
    for original in [
        Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(equality.clone()),
        },
        Proposition::Equal(right.clone(), left.clone()),
    ] {
        let proof = transport(
            goal.clone(),
            vec![ProofNode {
                conclusion: original.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }],
        );
        assert!(accept_certificate(&context(), &goal, &[original], &[], &proof).is_err());
    }
    let reversed = Proposition::Equal(right.clone(), left.clone());
    let proof = transport(
        goal.clone(),
        vec![ProofNode {
            conclusion: equality,
            rule: ProofRule::EqualitySymmetry {
                equality: Box::new(ProofNode {
                    conclusion: reversed.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        }],
    );
    assert!(accept_certificate(&context(), &goal, &[reversed], &[], &proof).is_ok());
}

#[test]
fn cyclic_and_expanding_equations_fail_with_bounded_work() {
    for equations in [
        vec![
            Proposition::Equal(value(1), value(2)),
            Proposition::Equal(value(2), value(1)),
        ],
        (1..=20)
            .map(|identity| {
                Proposition::Equal(
                    value(identity),
                    equal(value(identity + 1), value(identity + 1)),
                )
            })
            .collect(),
    ] {
        let goal = Proposition::Equal(value(1), value(1));
        let proof = transport(goal.clone(), axiom_proofs(&equations));
        assert!(accept_certificate(&context(), &goal, &[], &equations, &proof).is_err());
    }
}

#[test]
fn borrowed_equations_are_bounded_before_cloning_or_traversing_the_whole_roster() {
    use proof_admission::{PredicateDenotationError, check_value_equality_denotation};
    let equation = Proposition::Equal(value(1), value(2));
    // An unbounded borrowed iterator must terminate at the shared input limit,
    // without first materializing a roster or cloning all of its terms.
    assert_eq!(
        check_value_equality_denotation(
            &context(),
            &Proposition::Truth,
            std::iter::repeat(&equation)
        ),
        Err(PredicateDenotationError::ResourceLimitExceeded),
    );
    let mut deep = value(2);
    for _ in 0..65 {
        deep = ScalarTerm::BooleanNot {
            operand: Box::new(deep),
        };
    }
    let equation = Proposition::Equal(value(1), deep);
    assert_eq!(
        check_value_equality_denotation(&context(), &Proposition::Truth, [&equation]),
        Err(PredicateDenotationError::ResourceLimitExceeded),
    );
}

mod float_fields {
    use super::{ProofNode, ProofRule, accept_certificate, axiom_proofs, transport};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatValue,
        PlaceId, Proposition, PropositionContext, ScalarTerm, ScalarType, StructuralFieldId,
        StructuralPlaceKind, ValueId,
    };

    fn context(format: IeeeFloatFormat) -> PropositionContext {
        PropositionContext::from_value_types_and_places(
            (1..=2).map(|identity| {
                (
                    ValueId::new(identity).unwrap(),
                    ScalarType::IeeeFloat(format),
                )
            }),
            (1..=2).map(|identity| {
                (
                    PlaceId::new(identity).unwrap(),
                    StructuralPlaceKind::Parameter {
                        position: identity as u32 - 1,
                    },
                )
            }),
        )
        .unwrap()
    }

    fn field(root: u64, member: u64, format: IeeeFloatFormat) -> ScalarTerm {
        ScalarTerm::ieee_float_field_path(
            PlaceId::new(root).unwrap(),
            vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(member).unwrap(),
            )],
            format,
        )
    }

    fn value(identity: u64, format: IeeeFloatFormat) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(identity).unwrap(),
            ScalarType::IeeeFloat(format),
        )
    }

    fn proof(goal: Proposition, equations: &[Proposition]) -> ProofNode {
        let premise = match &goal {
            // Exact float identity remains an identity after denotation;
            // unlike a closed IEEE comparison, it does not become Truth.
            Proposition::Equal(_, right) => ProofNode {
                conclusion: Proposition::Equal(right.clone(), right.clone()),
                rule: ProofRule::Primitive(super::PrimitiveJudgment::ReflexiveEquality),
            },
            _ => super::truth(),
        };
        let mut proof = transport(goal, axiom_proofs(&equations));
        let ProofRule::ValueEqualityTransport {
            premise: supplied, ..
        } = &mut proof.rule
        else {
            unreachable!()
        };
        *supplied = Box::new(premise);
        proof
    }

    #[test]
    fn store_and_read_equations_transport_through_the_same_exact_field() {
        for literal in [
            IeeeFloatValue::Binary32(0x3f00_0000),
            IeeeFloatValue::Binary64(0x3fe0_0000_0000_0000),
        ] {
            let format = literal.format();
            let literal = ScalarTerm::IeeeFloatLiteral(literal);
            let equations = [
                Proposition::Equal(value(1, format), literal.clone()),
                Proposition::Equal(field(1, 1, format), value(1, format)),
                Proposition::Equal(value(2, format), field(1, 1, format)),
            ];
            for subject in [field(1, 1, format), value(2, format)] {
                let goal = Proposition::Equal(subject, literal.clone());
                let accepted = accept_certificate(
                    &context(format),
                    &goal,
                    &[],
                    &equations,
                    &proof(goal.clone(), &equations),
                )
                .unwrap();
                assert!(matches!(
                    accepted.mathematical_core,
                    proof_admission::MathematicalCoreDecision::Judged(_)
                ));
            }
            let goal = Proposition::Equal(value(2, format), literal);
            let mut missing = proof(goal.clone(), &equations);
            let ProofRule::ValueEqualityTransport { equalities, .. } = &mut missing.rule else {
                unreachable!()
            };
            equalities.remove(1);
            assert!(
                accept_certificate(&context(format), &goal, &[], &equations, &missing).is_err()
            );
        }
    }

    #[test]
    fn conditional_wrong_identity_and_wrong_format_equations_cannot_rewrite_a_field() {
        let format = IeeeFloatFormat::Binary32;
        let literal = ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary32(0x3f00_0000));
        let goal = Proposition::Equal(field(1, 1, format), literal.clone());
        for equation in [
            Proposition::Implication {
                premise: Box::new(Proposition::Falsehood),
                conclusion: Box::new(goal.clone()),
            },
            Proposition::Equal(field(2, 1, format), literal.clone()),
            Proposition::Equal(field(1, 2, format), literal),
            Proposition::Equal(
                field(1, 1, IeeeFloatFormat::Binary64),
                ScalarTerm::IeeeFloatLiteral(IeeeFloatValue::Binary64(0x3fe0_0000_0000_0000)),
            ),
        ] {
            let equations = [equation];
            assert!(
                accept_certificate(
                    &context(format),
                    &goal,
                    &[],
                    &equations,
                    &proof(goal.clone(), &equations)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn field_value_cycles_fail_with_bounded_work() {
        use proof_admission::{PredicateDenotationError, check_value_equality_denotation};
        let format = IeeeFloatFormat::Binary32;
        let equations = [
            Proposition::Equal(field(1, 1, format), value(1, format)),
            Proposition::Equal(value(1, format), field(1, 1, format)),
        ];
        let goal = Proposition::Equal(field(1, 1, format), field(1, 1, format));
        assert_eq!(
            check_value_equality_denotation(&context(format), &goal, &equations),
            Err(PredicateDenotationError::CyclicValueEquality)
        );
        assert!(
            accept_certificate(
                &context(format),
                &goal,
                &[],
                &equations,
                &proof(goal.clone(), &equations)
            )
            .is_err()
        );
    }

    #[test]
    fn field_bit_identity_does_not_become_ieee_equality() {
        for (bits, other_bits, ieee_equal) in [
            (
                IeeeFloatValue::Binary32(0x7fc0_1234),
                IeeeFloatValue::Binary32(0x7fc0_1235),
                false,
            ),
            (
                IeeeFloatValue::Binary64(0x7ff8_0000_0000_1234),
                IeeeFloatValue::Binary64(0x7ff8_0000_0000_1235),
                false,
            ),
            (
                IeeeFloatValue::Binary32(0x8000_0000),
                IeeeFloatValue::Binary32(0),
                true,
            ),
            (
                IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
                IeeeFloatValue::Binary64(0),
                true,
            ),
        ] {
            let format = bits.format();
            let subject = field(1, 1, format);
            let literal = ScalarTerm::IeeeFloatLiteral(bits);
            let equations = [Proposition::Equal(subject.clone(), literal.clone())];
            for (right, accepted) in [
                (literal, true),
                (ScalarTerm::IeeeFloatLiteral(other_bits), false),
            ] {
                let goal = Proposition::Equal(subject.clone(), right);
                assert_eq!(
                    accept_certificate(
                        &context(format),
                        &goal,
                        &[],
                        &equations,
                        &proof(goal.clone(), &equations)
                    )
                    .is_ok(),
                    accepted
                );
            }
            for kind in [
                IeeeFloatComparisonKind::Equal,
                IeeeFloatComparisonKind::NotEqual,
            ] {
                let goal = Proposition::ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left: subject.clone(),
                    right: subject.clone(),
                };
                let accepted = if kind == IeeeFloatComparisonKind::Equal {
                    ieee_equal
                } else {
                    !ieee_equal
                };
                assert_eq!(
                    accept_certificate(
                        &context(format),
                        &goal,
                        &[],
                        &equations,
                        &proof(goal.clone(), &equations)
                    )
                    .is_ok(),
                    accepted
                );
            }
        }
    }
}
