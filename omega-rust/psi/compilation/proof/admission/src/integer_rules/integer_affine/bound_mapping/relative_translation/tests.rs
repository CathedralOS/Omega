use crate::{
    IntegerAffineBoundConversionError, IntegerAffineWitness, PrimitiveJudgment, ProofNode,
    ProofRule, check_certificate, check_integer_affine_witness, integer_affine_wrapping_evidence,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

fn value(carrier: IntegerType, raw: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(raw).unwrap(), ScalarType::Integer(carrier))
}
fn literal(carrier: IntegerType, raw: u64) -> ScalarTerm {
    ScalarTerm::integer(
        carrier,
        match carrier.sign() {
            IntegerSign::Signed => IntegerValue::Signed(i128::from(raw)),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(raw)),
        },
    )
    .unwrap()
}
fn context(carrier: IntegerType) -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=4).map(|raw| (ValueId::new(raw).unwrap(), ScalarType::Integer(carrier))),
    )
    .unwrap()
}
fn operation(
    carrier: IntegerType,
    operand: u64,
    amount: u64,
    wrapping: bool,
    subtract: bool,
) -> ScalarTerm {
    let left = Box::new(value(carrier, operand));
    let right = Box::new(literal(carrier, amount));
    match (wrapping, subtract) {
        (true, false) => ScalarTerm::WrappingIntegerAdd {
            scalar_type: carrier,
            left,
            right,
        },
        (true, true) => ScalarTerm::WrappingIntegerSubtract {
            scalar_type: carrier,
            left,
            right,
        },
        (false, false) => ScalarTerm::ExactIntegerAdd {
            scalar_type: carrier,
            left,
            right,
        },
        (false, true) => ScalarTerm::ExactIntegerSubtract {
            scalar_type: carrier,
            left,
            right,
        },
    }
}
fn witness(carrier: IntegerType, end: u64, count: usize) -> IntegerAffineWitness {
    IntegerAffineWitness {
        root: value(carrier, 1),
        target: value(carrier, end),
        definition_axioms: (0..count).collect(),
        literal_axioms: vec![None; count],
    }
}
fn reflexive(root: &ScalarTerm) -> ProofNode {
    ProofNode {
        conclusion: Proposition::LessOrEqual(root.clone(), root.clone()),
        rule: ProofRule::IntegerOrderWeakening {
            relation: Box::new(ProofNode {
                conclusion: Proposition::Equal(root.clone(), root.clone()),
                rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
            }),
        },
    }
}
fn certificate(
    witness: IntegerAffineWitness,
    guards: &[Proposition],
    goal: Proposition,
) -> ProofNode {
    let base = reflexive(&witness.root);
    let root_bound = if guards.is_empty() {
        base
    } else {
        let mut children = vec![base];
        children.extend(guards.iter().enumerate().map(|(index, guard)| ProofNode {
            conclusion: guard.clone(),
            rule: ProofRule::Assumption { index },
        }));
        ProofNode {
            conclusion: Proposition::Conjunction(
                children
                    .iter()
                    .map(|child| child.conclusion.clone())
                    .collect(),
            ),
            rule: ProofRule::ConjunctionIntroduction(children),
        }
    };
    ProofNode {
        conclusion: goal,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    }
}
fn bounded_root(root: &ScalarTerm, guards: &[Proposition]) -> Proposition {
    let base = Proposition::LessOrEqual(root.clone(), root.clone());
    if guards.is_empty() {
        base
    } else {
        Proposition::Conjunction(
            std::iter::once(base)
                .chain(guards.iter().cloned())
                .collect(),
        )
    }
}

#[test]
fn relative_additive_translation_certifies_signed_and_unsigned_progress() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let carrier = IntegerType::new(sign, 64).unwrap();
        for wrapping in [false, true] {
            for subtract in [false, true] {
                let context = context(carrier);
                let axioms = [Proposition::Equal(
                    value(carrier, 2),
                    operation(carrier, 1, 1, wrapping, subtract),
                )];
                let witness = witness(carrier, 2, 1);
                let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
                let guards =
                    integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion)
                        .unwrap();
                assert_eq!(guards.len(), usize::from(wrapping));
                let goal =
                    map_integer_affine_bound(&form, &bounded_root(&witness.root, &guards)).unwrap();
                let expected = if subtract {
                    Proposition::LessThan(witness.target.clone(), witness.root.clone())
                } else {
                    Proposition::LessThan(witness.root.clone(), witness.target.clone())
                };
                assert_eq!(goal, expected);
                let proof = certificate(witness, &guards, goal);
                check_certificate(&context, &proof.conclusion, &guards, &axioms, &proof).unwrap();
                if wrapping {
                    assert!(
                        check_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
                            .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn relative_additive_translation_requires_each_intermediate_wrapping_guard() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let axioms = [
        Proposition::Equal(value(carrier, 2), operation(carrier, 1, 2, true, false)),
        Proposition::Equal(value(carrier, 3), operation(carrier, 2, 3, true, true)),
    ];
    let witness = witness(carrier, 3, 2);
    let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
    let guards =
        integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion).unwrap();
    assert_eq!(guards.len(), 2);
    assert_eq!(
        guards[1],
        Proposition::LessOrEqual(literal(carrier, 3), value(carrier, 2))
    );
    let goal = map_integer_affine_bound(&form, &bounded_root(&witness.root, &guards)).unwrap();
    let proof = certificate(witness.clone(), &guards, goal.clone());
    check_certificate(&context, &goal, &guards, &axioms, &proof).unwrap();
    for dropped in 0..2 {
        let mut incomplete = guards.clone();
        incomplete.remove(dropped);
        let forged = certificate(witness.clone(), &incomplete, goal.clone());
        assert!(check_certificate(&context, &goal, &incomplete, &axioms, &forged).is_err());
    }
    let mut wrong = guards.clone();
    wrong[1] = Proposition::LessOrEqual(literal(carrier, 3), value(carrier, 1));
    let forged = certificate(witness, &wrong, goal.clone());
    assert!(check_certificate(&context, &goal, &wrong, &axioms, &forged).is_err());
}

#[test]
fn relative_additive_translation_inverse_keeps_the_original_operand_guard() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let axioms = [Proposition::Equal(
        value(carrier, 2),
        operation(carrier, 1, 1, true, false),
    )];
    let mut witness = witness(carrier, 1, 1);
    witness.root = value(carrier, 2);
    let form = check_integer_affine_witness(&context, &axioms, &witness).unwrap();
    let guards =
        integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion).unwrap();
    let goal = map_integer_affine_bound(&form, &bounded_root(&witness.root, &guards)).unwrap();
    assert_eq!(
        goal,
        Proposition::LessThan(value(carrier, 1), value(carrier, 2))
    );
    let proof = certificate(witness, &guards, goal);
    check_certificate(&context, &proof.conclusion, &guards, &axioms, &proof).unwrap();
}

#[test]
fn relative_additive_translation_refuses_zero_scaling_and_carrier_drift() {
    let carrier = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let context = context(carrier);
    let zero_axioms = [
        Proposition::Equal(value(carrier, 2), operation(carrier, 1, 1, true, false)),
        Proposition::Equal(value(carrier, 3), operation(carrier, 2, 1, true, true)),
    ];
    let witness = witness(carrier, 3, 2);
    let form = check_integer_affine_witness(&context, &zero_axioms, &witness).unwrap();
    assert_eq!(
        integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion),
        Err(IntegerAffineBoundConversionError::NonProgressingTranslation)
    );
    let scaling = [Proposition::Equal(
        value(carrier, 2),
        ScalarTerm::ExactIntegerMultiply {
            scalar_type: carrier,
            left: Box::new(value(carrier, 1)),
            right: Box::new(literal(carrier, 2)),
        },
    )];
    let mut witness = witness;
    witness.target = value(carrier, 2);
    witness.definition_axioms = vec![0];
    witness.literal_axioms = vec![None];
    let form = check_integer_affine_witness(&context, &scaling, &witness).unwrap();
    assert_eq!(
        integer_affine_wrapping_evidence(&form, &reflexive(&witness.root).conclusion),
        Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation)
    );
    witness.root = value(IntegerType::new(IntegerSign::Unsigned, 32).unwrap(), 1);
    assert!(check_integer_affine_witness(&context, &scaling, &witness).is_err());
}
