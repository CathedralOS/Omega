use super::super::Elaboration;
use crate::{Budget, IntegerAffineWitness, ProofNode, ProofRule, verify_bounded_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

#[test]
fn correlated_add_upper_uses_fixed_laws_and_checks_original_operands() {
    check_correlated_add_bound(IntegerType::new(IntegerSign::Unsigned, 64).unwrap(), false);
}

#[test]
fn correlated_add_lower_uses_fixed_laws_and_checks_original_operands() {
    for width in [8, 32, 64, 128] {
        check_correlated_add_bound(IntegerType::new(IntegerSign::Signed, width).unwrap(), true);
    }
}

fn check_correlated_add_bound(integer: IntegerType, lower: bool) {
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
    let endpoint = if lower {
        integer.minimum_value()
    } else {
        integer.maximum_value()
    };
    let endpoint_term = ScalarTerm::integer(integer, endpoint).unwrap();
    let root =
        ScalarTerm::exact_integer_subtract(integer, endpoint_term.clone(), value(2)).unwrap();
    let premise = if lower {
        Proposition::LessOrEqual(root.clone(), value(1))
    } else {
        Proposition::LessOrEqual(value(1), root.clone())
    };
    let sum = IntegerMathTerm::Add(Box::new(math(1)), Box::new(math(2)));
    let endpoint_math = IntegerMathTerm::literal(endpoint);
    let goal = if lower {
        Proposition::IntegerMathLessOrEqual(endpoint_math, sum)
    } else {
        Proposition::IntegerMathLessOrEqual(sum, endpoint_math)
    };
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
        ScalarTerm::exact_integer_subtract(integer, endpoint_term, value(3)).unwrap(),
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
fn correlated_add_bounds_transport_ssa_roots_and_carrier_endpoints() {
    for (sign, width, lower) in [
        (IntegerSign::Unsigned, 8, false),
        (IntegerSign::Unsigned, 64, false),
        (IntegerSign::Unsigned, 128, false),
        (IntegerSign::Signed, 128, false),
        (IntegerSign::Signed, 32, true),
        (IntegerSign::Signed, 64, true),
        (IntegerSign::Signed, 128, true),
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
        let premise = if lower {
            Proposition::LessOrEqual(value(3), left.clone())
        } else {
            Proposition::LessOrEqual(left.clone(), value(3))
        };
        let endpoint = if lower {
            integer.minimum_value()
        } else {
            integer.maximum_value()
        };
        let endpoint = ScalarTerm::integer(integer, endpoint).unwrap();
        let difference = ScalarTerm::exact_integer_subtract(integer, value(4), value(2)).unwrap();
        let sum = IntegerMathTerm::Add(
            Box::new(mathematical(&left)),
            Box::new(mathematical(&value(2))),
        );
        let goal = if lower {
            Proposition::IntegerMathLessOrEqual(mathematical(&endpoint), sum)
        } else {
            Proposition::IntegerMathLessOrEqual(sum, mathematical(&endpoint))
        };
        for reversed in [false, true] {
            let equality = |left, right| {
                if reversed {
                    Proposition::Equal(right, left)
                } else {
                    Proposition::Equal(left, right)
                }
            };
            let axioms = [
                equality(value(4), endpoint.clone()),
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

fn closed_addend_rule_axioms(
    integer: IntegerType,
    lower: bool,
    left: IntegerValue,
    right: IntegerValue,
) -> usize {
    let literal = |value| ScalarTerm::integer(integer, value).unwrap();
    let endpoint = if lower {
        integer.minimum_value()
    } else {
        integer.maximum_value()
    };
    let left = literal(left);
    let right = literal(right);
    let root =
        ScalarTerm::exact_integer_subtract(integer, literal(endpoint), right.clone()).unwrap();
    let premise = if lower {
        Proposition::LessOrEqual(root.clone(), left.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), root.clone())
    };
    let sum = IntegerMathTerm::Add(
        Box::new(mathematical(&left)),
        Box::new(mathematical(&right)),
    );
    let goal = if lower {
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::literal(endpoint), sum)
    } else {
        Proposition::IntegerMathLessOrEqual(sum, IntegerMathTerm::literal(endpoint))
    };
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
    assert!(
        elaboration.denotation.addition.laws.is_empty(),
        "closed goals derive from numeral laws, not the law chain"
    );
    elaboration.denotation.rule_axioms.len()
}

#[test]
fn closed_addend_bounds_derive_from_numeral_laws() {
    let wide = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    for (left, right) in [(0, 0), (1, 2), (u128::MAX, 0), (0, u128::MAX)] {
        assert_eq!(
            closed_addend_rule_axioms(
                wide,
                false,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ),
            0
        );
    }
    let signed = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    for (left, right) in [
        (IntegerValue::Signed(-100), IntegerValue::Signed(10)),
        (IntegerValue::Signed(-118), IntegerValue::Signed(-10)),
    ] {
        assert_eq!(closed_addend_rule_axioms(signed, true, left, right), 0);
    }
}

#[test]
fn closed_addend_false_goal_empties_the_checked_premise() {
    // `u128::MAX + u128::MAX <= u128::MAX` is a false relation between
    // numerals; the checked premise `MAX <= MAX - MAX` is contradictory, so
    // empty elimination derives the goal without an instance axiom.
    let wide = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    assert_eq!(
        closed_addend_rule_axioms(
            wide,
            false,
            IntegerValue::Unsigned(u128::MAX),
            IntegerValue::Unsigned(u128::MAX),
        ),
        0
    );
    // The same bridge inside the carrier: `200 + 100 > 255` over u8.
    let narrow = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(
        closed_addend_rule_axioms(
            narrow,
            false,
            IntegerValue::Unsigned(200),
            IntegerValue::Unsigned(100),
        ),
        0
    );
}

#[test]
fn correlated_add_closed_addend_keeps_open_sum_on_the_law_chain() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let context = PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let endpoint = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
    let decrement = ScalarTerm::integer(integer, IntegerValue::Unsigned(7)).unwrap();
    let difference =
        ScalarTerm::exact_integer_subtract(integer, value(4), decrement.clone()).unwrap();
    let premise = Proposition::LessOrEqual(value(1), value(3));
    let sum = IntegerMathTerm::Add(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&decrement)),
    );
    let goal = Proposition::IntegerMathLessOrEqual(sum, mathematical(&endpoint));
    for reversed in [false, true] {
        let equality = |left, right| {
            if reversed {
                Proposition::Equal(right, left)
            } else {
                Proposition::Equal(left, right)
            }
        };
        let axioms = [
            equality(value(4), endpoint.clone()),
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
                    target: ScalarTerm::exact_integer_add(integer, value(1), decrement.clone())
                        .unwrap(),
                    definition_axioms: vec![1],
                    literal_axioms: vec![Some(0)],
                },
            },
        };
        verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&premise),
            &axioms,
            &proof,
            &mut Budget::default(),
        )
        .unwrap();
        let parameters = BTreeSet::new();
        let assumptions = [premise.clone()];
        let mut elaboration =
            Elaboration::new(&context, &goal, &assumptions, &axioms, &parameters).unwrap();
        elaboration.node(&proof).unwrap();
        assert!(elaboration.denotation.rule_axioms.is_empty());
        assert_eq!(elaboration.denotation.addition.laws.len(), 2);
    }
}

#[test]
fn open_sum_over_closed_difference_uses_the_numeral_equation() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Unsigned(value)).unwrap();
    let right = literal(7);
    let root = ScalarTerm::exact_integer_subtract(integer, literal(255), right.clone()).unwrap();
    let premise = Proposition::LessOrEqual(left.clone(), root.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(mathematical(&left)),
            Box::new(mathematical(&right)),
        ),
        mathematical(&literal(255)),
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
    // The closed difference denotes to its numeral `248`, so the
    // cancellation step uses the interned numeral-operation equation
    // `add 248 7 = 255`; monotonicity and endpoint substitution re-decide
    // the rest — no instance axiom.
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert_eq!(elaboration.denotation.addition.laws.len(), 1);
    assert!(
        elaboration
            .denotation
            .addition
            .laws
            .contains_key(&super::Law::Monotone)
    );
    assert_eq!(elaboration.denotation.addition.numeral_sums.len(), 1);
    assert_eq!(
        elaboration
            .denotation
            .addition
            .numeral_sums
            .keys()
            .next()
            .unwrap(),
        &(
            IntegerValue::Unsigned(248),
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(255)
        )
    );
    // A tampered premise or conclusion rejects at the shared relation
    // before denotation — the equation can only be `add 248 7 = 255`.
    let mut wrong_premise = proof.clone();
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut wrong_premise.rule else {
        unreachable!()
    };
    root_bound.conclusion = Proposition::LessOrEqual(
        ScalarTerm::value(ValueId::new(1).unwrap(), scalar),
        literal(249),
    );
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            &assumptions,
            &[],
            &wrong_premise,
            &mut Budget::default()
        )
        .is_err()
    );
    let mut wrong_goal = proof.clone();
    wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(mathematical(&ScalarTerm::value(
                ValueId::new(1).unwrap(),
                scalar,
            ))),
            Box::new(mathematical(&literal(7))),
        ),
        mathematical(&literal(254)),
    );
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            &assumptions,
            &[],
            &wrong_goal,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn open_sum_over_signed_closed_difference_uses_the_numeral_equation() {
    // The lower-bound direction substitutes the left endpoint of the
    // order, and the signed numerals exercise the `negate` prefix:
    // `add (-121) (-7) = (-128)` bridges `min ≤ x + (-7)` after
    // monotonicity on `-121 ≤ x`.
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let right = literal(-7);
    let root = ScalarTerm::exact_integer_subtract(integer, literal(-128), right.clone()).unwrap();
    let premise = Proposition::LessOrEqual(root.clone(), left.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        mathematical(&literal(-128)),
        IntegerMathTerm::Add(
            Box::new(mathematical(&left)),
            Box::new(mathematical(&right)),
        ),
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
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert_eq!(
        elaboration
            .denotation
            .addition
            .numeral_sums
            .keys()
            .collect::<Vec<_>>(),
        [&(
            IntegerValue::Signed(-121),
            IntegerValue::Signed(-7),
            IntegerValue::Signed(-128)
        )]
    );
}

/// The `IntegerExactAddDefinitionBound` fixture: two operand bounds plus a
/// cited `out = l + r` definition prove `k ≤ out` or `out ≤ k`. Returns the
/// elaboration's denotation state for assertions after the shared checker
/// and kernel have both accepted.
fn exact_add_definition_fixture(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    proof: &ProofNode,
) -> (crate::BoundedDenotation, super::super::Denotation) {
    let denoted = verify_bounded_certificate(
        context,
        goal,
        assumptions,
        axioms,
        proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(context, goal, assumptions, axioms, &parameters).unwrap();
    elaboration.node(proof).unwrap();
    (denoted, elaboration.denotation)
}

fn exact_add_definition_proof(
    left_premise: &Proposition,
    right_premise: &Proposition,
    definition_axiom: usize,
    goal: &Proposition,
) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerExactAddDefinitionBound {
            left_bound: Box::new(ProofNode {
                conclusion: left_premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            right_bound: Box::new(ProofNode {
                conclusion: right_premise.clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
            definition_axiom,
        },
    }
}

fn check_exact_add_definition_bound(integer: IntegerType, lower: bool, reversed: bool) {
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let (left_endpoint, right_endpoint, bound) = match (integer.sign(), lower) {
        (IntegerSign::Signed, true) => (
            IntegerValue::Signed(-5),
            IntegerValue::Signed(-3),
            IntegerValue::Signed(-8),
        ),
        (IntegerSign::Signed, false) => (
            IntegerValue::Signed(7),
            IntegerValue::Signed(9),
            IntegerValue::Signed(16),
        ),
        (IntegerSign::Unsigned, true) => (
            IntegerValue::Unsigned(2),
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(5),
        ),
        (IntegerSign::Unsigned, false) => (
            IntegerValue::Unsigned(100),
            IntegerValue::Unsigned(50),
            IntegerValue::Unsigned(150),
        ),
    };
    let literal = |value| ScalarTerm::integer(integer, value).unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
    let (left_premise, right_premise) = if lower {
        (
            Proposition::LessOrEqual(literal(left_endpoint), value(1)),
            Proposition::LessOrEqual(literal(right_endpoint), value(2)),
        )
    } else {
        (
            Proposition::LessOrEqual(value(1), literal(left_endpoint)),
            Proposition::LessOrEqual(value(2), literal(right_endpoint)),
        )
    };
    let definition = if reversed {
        Proposition::Equal(expression.clone(), output.clone())
    } else {
        Proposition::Equal(output.clone(), expression.clone())
    };
    let goal = if lower {
        Proposition::LessOrEqual(literal(bound), output.clone())
    } else {
        Proposition::LessOrEqual(output.clone(), literal(bound))
    };
    let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
    let assumptions = [left_premise.clone(), right_premise.clone()];
    let axioms = [definition.clone()];
    let (denoted, denotation) =
        exact_add_definition_fixture(&context, &goal, &assumptions, &axioms, &proof);
    assert!(denoted.certificate.signature.len() < 400);
    assert!(
        denotation.rule_axioms.is_empty(),
        "exact-add definition bounds derive from the fixed monotone law"
    );
    assert!(denotation.decisions.is_empty());
    assert_eq!(denotation.addition.laws.len(), 1);
    assert_eq!(
        denotation.addition.numeral_sums.keys().collect::<Vec<_>>(),
        [&(left_endpoint, right_endpoint, bound)]
    );
}

#[test]
fn exact_add_definition_bounds_use_the_two_sided_monotone_law() {
    for (sign, width) in [
        (IntegerSign::Unsigned, 8),
        (IntegerSign::Unsigned, 64),
        (IntegerSign::Signed, 32),
        (IntegerSign::Signed, 128),
    ] {
        for lower in [true, false] {
            for reversed in [false, true] {
                check_exact_add_definition_bound(
                    IntegerType::new(sign, width).unwrap(),
                    lower,
                    reversed,
                );
            }
        }
    }
}

#[test]
fn exact_add_definition_bound_transports_operand_equalities() {
    // `x = 4 ∧ 3 ≤ y` with `out = x + y` proves `7 ≤ out`: the equality
    // endpoint re-shapes to `IntLe` through `eq_le` in either citation
    // orientation. The upper symmetric shape `x = 4 ∧ y ≤ 9` proves
    // `out ≤ 13`.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
    for reversed_equality in [false, true] {
        for lower in [true, false] {
            let (left_premise, right_premise, bound) = if lower {
                let equality = if reversed_equality {
                    Proposition::Equal(literal(4), value(1))
                } else {
                    Proposition::Equal(value(1), literal(4))
                };
                (
                    equality,
                    Proposition::LessOrEqual(literal(3), value(2)),
                    literal(7),
                )
            } else {
                let equality = if reversed_equality {
                    Proposition::Equal(literal(4), value(1))
                } else {
                    Proposition::Equal(value(1), literal(4))
                };
                (
                    equality,
                    Proposition::LessOrEqual(value(2), literal(9)),
                    literal(13),
                )
            };
            let definition = Proposition::Equal(output.clone(), expression.clone());
            let goal = if lower {
                Proposition::LessOrEqual(bound.clone(), output.clone())
            } else {
                Proposition::LessOrEqual(output.clone(), bound.clone())
            };
            let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
            let assumptions = [left_premise, right_premise];
            let axioms = [definition];
            let (_, denotation) =
                exact_add_definition_fixture(&context, &goal, &assumptions, &axioms, &proof);
            assert!(denotation.rule_axioms.is_empty());
            assert_eq!(denotation.addition.laws.len(), 1);
        }
    }
}

#[test]
fn exact_add_definition_bound_closed_addend_uses_refl() {
    // `Truth` over the literal addend `2` (its own endpoint through `refl`)
    // plus `y ≤ 9` proves `out ≤ 11` for `out = 2 + y`.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_add(integer, literal(2), value(2)).unwrap();
    let right_premise = Proposition::LessOrEqual(value(2), literal(9));
    let definition = Proposition::Equal(output.clone(), expression.clone());
    let goal = Proposition::LessOrEqual(output.clone(), literal(11));
    let proof = exact_add_definition_proof(&Proposition::Truth, &right_premise, 0, &goal);
    let assumptions = [Proposition::Truth, right_premise];
    let axioms = [definition];
    let (_, denotation) =
        exact_add_definition_fixture(&context, &goal, &assumptions, &axioms, &proof);
    assert!(denotation.rule_axioms.is_empty());
    assert_eq!(denotation.addition.laws.len(), 1);
    assert_eq!(
        denotation.addition.numeral_sums.keys().collect::<Vec<_>>(),
        [&(
            IntegerValue::Unsigned(2),
            IntegerValue::Unsigned(9),
            IntegerValue::Unsigned(11)
        )]
    );
}

#[test]
fn exact_add_definition_bound_carrier_endpoint_keeps_instance_fallback() {
    // `Truth` over an open addend contributes only its carrier endpoint;
    // no interned carrier-bound law exists, so the checked instance axiom
    // still names this shape — `127 + (-5) = 122` over i8.
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Signed(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
    let right_premise = Proposition::LessOrEqual(value(2), literal(-5));
    let definition = Proposition::Equal(output.clone(), expression.clone());
    let goal = Proposition::LessOrEqual(output.clone(), literal(122));
    let proof = exact_add_definition_proof(&Proposition::Truth, &right_premise, 0, &goal);
    let assumptions = [Proposition::Truth, right_premise];
    let axioms = [definition];
    let (_, denotation) =
        exact_add_definition_fixture(&context, &goal, &assumptions, &axioms, &proof);
    assert_eq!(
        denotation.rule_axioms.len(),
        1,
        "carrier endpoints keep the checked instance axiom"
    );
}

#[test]
fn exact_add_definition_bound_rejects_mismatched_certificates() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap();
    let left_premise = Proposition::LessOrEqual(literal(2), value(1));
    let right_premise = Proposition::LessOrEqual(literal(3), value(2));
    let definition = Proposition::Equal(output.clone(), expression.clone());
    let goal = Proposition::LessOrEqual(literal(5), output.clone());
    let proof = exact_add_definition_proof(&left_premise, &right_premise, 0, &goal);
    let assumptions = [left_premise, right_premise];
    let axioms = [definition];
    // A wrong bound literal, a wrong output, a bound on the wrong operand,
    // and a definition naming a different addend all fail the shared
    // relation check before denotation.
    for wrong_goal in [
        Proposition::LessOrEqual(literal(6), output.clone()),
        Proposition::LessOrEqual(literal(5), value(4)),
        Proposition::LessOrEqual(literal(5), expression.clone()),
    ] {
        let mut invalid = proof.clone();
        invalid.conclusion = wrong_goal.clone();
        assert!(
            verify_bounded_certificate(
                &context,
                &wrong_goal,
                &assumptions,
                &axioms,
                &invalid,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    for wrong_premise in [
        Proposition::LessOrEqual(literal(2), value(2)),
        Proposition::LessOrEqual(literal(3), value(1)),
        Proposition::LessOrEqual(value(1), literal(2)),
        Proposition::Equal(value(1), value(2)),
    ] {
        let mut invalid = proof.clone();
        let ProofRule::IntegerExactAddDefinitionBound { left_bound, .. } = &mut invalid.rule else {
            unreachable!()
        };
        left_bound.conclusion = wrong_premise.clone();
        let wrong_assumptions = [wrong_premise, assumptions[1].clone()];
        assert!(
            verify_bounded_certificate(
                &context,
                &goal,
                &wrong_assumptions,
                &axioms,
                &invalid,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    // A definition citing a different output or different addends.
    for wrong_axiom in [
        Proposition::Equal(value(4), expression.clone()),
        Proposition::Equal(
            output.clone(),
            ScalarTerm::exact_integer_add(integer, value(1), value(1)).unwrap(),
        ),
        Proposition::LessOrEqual(output.clone(), expression.clone()),
    ] {
        assert!(
            verify_bounded_certificate(
                &context,
                &goal,
                &assumptions,
                std::slice::from_ref(&wrong_axiom),
                &proof,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    // An out-of-roster citation.
    let mut invalid = proof.clone();
    let ProofRule::IntegerExactAddDefinitionBound {
        definition_axiom, ..
    } = &mut invalid.rule
    else {
        unreachable!()
    };
    *definition_axiom = 7;
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            &assumptions,
            &axioms,
            &invalid,
            &mut Budget::default()
        )
        .is_err()
    );
}
