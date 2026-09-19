use super::super::Elaboration;
use crate::{Budget, IntegerAffineWitness, ProofNode, ProofRule, verify_bounded_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

#[test]
fn correlated_add_upper_uses_fixed_laws_and_checks_original_operands() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let math = |index| IntegerMathTerm::MathValue {
        source_type: integer,
        value: ValueId::new(index).unwrap(),
    };
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let maximum = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
    let root = ScalarTerm::exact_integer_subtract(integer, maximum, value(2)).unwrap();
    let premise = Proposition::LessOrEqual(value(1), root.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(Box::new(math(1)), Box::new(math(2))),
        IntegerMathTerm::literal(integer.maximum_value()),
    );
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness: IntegerAffineWitness {
                root,
                target: ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let assumptions = [premise];
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(
        elaboration.denotation.rule_axioms.is_empty(),
        "correlated addition must derive its endpoint from fixed laws"
    );
    assert!(elaboration.denotation.decisions.is_empty());
    assert_eq!(elaboration.denotation.addition.laws.len(), 2);
    for wrong_root in [
        value(3),
        ScalarTerm::exact_integer_subtract(
            integer,
            ScalarTerm::integer(integer, IntegerValue::Unsigned(u64::MAX as u128)).unwrap(),
            value(3),
        )
        .unwrap(),
    ] {
        let mut invalid = proof.clone();
        let ProofRule::IntegerAffineBound { witness, .. } = &mut invalid.rule else {
            unreachable!()
        };
        witness.root = wrong_root;
        assert!(
            verify_bounded_certificate(
                &context,
                &goal,
                &assumptions,
                &[],
                &invalid,
                &mut Budget::default()
            )
            .is_err()
        );
    }
}

fn mathematical(term: &ScalarTerm) -> IntegerMathTerm {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(source_type),
        } => IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *id,
        },
        ScalarTerm::Integer { value, .. } => IntegerMathTerm::literal(*value),
        _ => unreachable!(),
    }
}

#[test]
fn open_mathematical_addition_shares_scalar_terms_without_prefix_keys() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let goal = Proposition::Equal(value(1), value(1));
    let mut elaboration = Elaboration::new(&context, &goal, &[], &[], &parameters).unwrap();
    let mut scalar_term = value(1);
    let mut math_term = mathematical(&scalar_term);
    for depth in 1..=64 {
        scalar_term = ScalarTerm::exact_integer_add(integer, scalar_term, value(2)).unwrap();
        math_term = IntegerMathTerm::Add(Box::new(math_term), Box::new(mathematical(&value(2))));
        if depth == 32 || depth == 64 {
            let before = elaboration.denotation.arena.len();
            let denoted_math = elaboration.denotation.math_term(&math_term).unwrap();
            let slots = elaboration.denotation.arena.len() - before;
            let denoted_scalar = elaboration
                .denotation
                .fixed_scalar_term(&scalar_term)
                .unwrap();
            assert!(
                elaboration
                    .denotation
                    .arena
                    .structurally_equal(denoted_math, denoted_scalar)
            );
            // Applications retain shallow handles; only the two leaf values
            // need source keys. Evaluator preflights still revisit prefixes.
            assert_eq!(elaboration.denotation.math_terms.len(), 2);
            assert!(elaboration.denotation.scalar_integer_terms.is_empty());
            assert!(slots < 5 * depth + 12);
            eprintln!("open addition depth={depth}, new mathematical slots={slots}");
        }
    }
}

#[test]
fn addition_identity_transport_accepts_existing_leaf_math_citations() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let compound = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
    let first = Proposition::Equal(compound.clone(), value(1));
    let second = Proposition::Equal(value(1), value(2));
    let math_second =
        Proposition::IntegerMathEqual(mathematical(&value(1)), mathematical(&value(2)));
    let goal = Proposition::Equal(compound, value(2));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(ProofNode {
                conclusion: first.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            middle_equals_right: Box::new(ProofNode {
                conclusion: second,
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    verify_bounded_certificate(
        &context,
        &goal,
        &[first, math_second],
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
}

#[test]
fn open_addition_preserves_previously_skipped_numeric_resource_refusals() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = ValueId::new(1).unwrap();
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
    let large = IntegerMathTerm::ShiftLeft {
        value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
        count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(65_536))),
    };
    let open = IntegerMathTerm::Add(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value,
        }),
        Box::new(large.clone()),
    );
    let goal = Proposition::IntegerMathEqual(open.clone(), open);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&goal),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    let closed_goal = Proposition::IntegerMathEqual(large.clone(), large);
    let closed_proof = ProofNode {
        conclusion: closed_goal.clone(),
        rule: ProofRule::Assumption { index: 0 },
    };
    assert!(
        verify_bounded_certificate(
            &context,
            &closed_goal,
            std::slice::from_ref(&closed_goal),
            &[],
            &closed_proof,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn correlated_add_upper_transports_ssa_root_and_maximum() {
    for (sign, width) in [
        (IntegerSign::Unsigned, 8),
        (IntegerSign::Unsigned, 64),
        (IntegerSign::Unsigned, 128),
        (IntegerSign::Signed, 128),
    ] {
        let integer = IntegerType::new(sign, width).unwrap();
        let scalar = ScalarType::Integer(integer);
        let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
        let context = PropositionContext::from_value_types(
            (1..=4).map(|index| (ValueId::new(index).unwrap(), scalar)),
        )
        .unwrap();
        let left = if width == 8 {
            ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap()
        } else {
            value(1)
        };
        let premise = Proposition::LessOrEqual(left.clone(), value(3));
        let maximum = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
        let difference = ScalarTerm::exact_integer_subtract(integer, value(4), value(2)).unwrap();
        let goal = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(mathematical(&left)),
                Box::new(mathematical(&value(2))),
            ),
            mathematical(&maximum),
        );
        for reversed in [false, true] {
            let equality = |left, right| {
                if reversed {
                    Proposition::Equal(right, left)
                } else {
                    Proposition::Equal(left, right)
                }
            };
            let axioms = [
                equality(value(4), maximum.clone()),
                equality(value(3), difference.clone()),
            ];
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(ProofNode {
                        conclusion: premise.clone(),
                        rule: ProofRule::Assumption { index: 0 },
                    }),
                    witness: IntegerAffineWitness {
                        root: value(3),
                        target: ScalarTerm::exact_integer_add(integer, left.clone(), value(2))
                            .unwrap(),
                        definition_axioms: vec![1],
                        literal_axioms: vec![Some(0)],
                    },
                },
            };
            let denoted = verify_bounded_certificate(
                &context,
                &goal,
                std::slice::from_ref(&premise),
                &axioms,
                &proof,
                &mut Budget::default(),
            )
            .unwrap();
            assert!(denoted.certificate.signature.len() < 300);
            let parameters = BTreeSet::new();
            let assumptions = [premise.clone()];
            let mut elaboration =
                Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
            elaboration.node(&proof).unwrap();
            assert!(elaboration.denotation.rule_axioms.is_empty());
        }
    }
}

#[test]
fn closed_addends_keep_canonical_values_and_explicit_instance_fallback() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    for (left_value, right_value) in [
        (0, 0),
        (1, 2),
        (u128::MAX, 0),
        (0, u128::MAX),
        (u128::MAX, u128::MAX),
    ] {
        let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
        let left = literal(left_value);
        let right = literal(right_value);
        let root =
            ScalarTerm::exact_integer_subtract(integer, literal(u128::MAX), right.clone()).unwrap();
        let premise = Proposition::LessOrEqual(left.clone(), root.clone());
        let goal = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(
                Box::new(mathematical(&left)),
                Box::new(mathematical(&right)),
            ),
            mathematical(&literal(u128::MAX)),
        );
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                witness: IntegerAffineWitness {
                    root,
                    target: ScalarTerm::exact_integer_add(integer, left, right).unwrap(),
                    definition_axioms: Vec::new(),
                    literal_axioms: Vec::new(),
                },
            },
        };
        let context = PropositionContext::from_value_types([]).unwrap();
        let assumptions = [premise];
        verify_bounded_certificate(
            &context,
            &goal,
            &assumptions,
            &[],
            &proof,
            &mut Budget::default(),
        )
        .unwrap();
        let parameters = BTreeSet::new();
        let mut elaboration =
            Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
        elaboration.node(&proof).unwrap();
        assert_eq!(elaboration.denotation.rule_axioms.len(), 1);
        assert!(elaboration.denotation.addition.laws.is_empty());
    }
}
