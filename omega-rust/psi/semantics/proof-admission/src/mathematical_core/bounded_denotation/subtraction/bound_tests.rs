use std::collections::BTreeSet;

use super::super::Elaboration;
use crate::{Budget, IntegerAffineWitness, ProofNode, ProofRule, verify_bounded_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};

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

fn check(left: ScalarTerm, right: ScalarTerm) {
    let ScalarType::Integer(integer) = left.scalar_type() else {
        unreachable!()
    };
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), left.scalar_type())),
    )
    .unwrap();
    let premise = Proposition::LessOrEqual(right.clone(), left.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
        IntegerMathTerm::Subtract(
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
                root: right.clone(),
                target: ScalarTerm::exact_integer_subtract(integer, left.clone(), right.clone())
                    .unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    let assumptions = [premise];
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(&context, &goal, &assumptions, &[], &parameters).unwrap();
    elaboration.node(&proof).unwrap();
    assert!(elaboration.denotation.rule_axioms.is_empty());
    assert!(elaboration.denotation.decisions.is_empty());
    if left.integer_value().is_none() || right.integer_value().is_none() {
        assert_eq!(elaboration.denotation.subtraction.laws.len(), 2);
        assert!(
            elaboration
                .denotation
                .subtraction
                .laws
                .contains_key(&super::Law::SelfZero)
        );
        assert!(
            elaboration
                .denotation
                .subtraction
                .laws
                .contains_key(&super::Law::LessOrEqualAntitone)
        );
    }

    let mut budget = Budget::default();
    let denoted =
        verify_bounded_certificate(&context, &goal, &assumptions, &[], &proof, &mut budget)
            .unwrap();
    eprintln!(
        "unsigned subtract bound {:?}, remaining={}",
        denoted.receipt(),
        budget.remaining()
    );
    assert!(denoted.certificate.signature.len() < 800);
    assert!(
        verify_bounded_certificate(&context, &goal, &[], &[], &proof, &mut Budget::default())
            .is_err()
    );
    let mut reversed = proof.clone();
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut reversed.rule else {
        unreachable!()
    };
    root_bound.conclusion = Proposition::LessOrEqual(left, right);
    let reversed_premise = root_bound.conclusion.clone();
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&reversed_premise),
            &[],
            &reversed,
            &mut Budget::default()
        )
        .is_err()
    );
}

#[test]
fn unsigned_subtraction_bounds_preserve_open_and_closed_conditional_domain() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    let value =
        |index| ScalarTerm::value(ValueId::new(index).unwrap(), ScalarType::Integer(integer));
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    check(value(1), value(2));
    check(value(1), literal(2));
    check(literal(2), value(2));
    for (left, right) in [
        (2, 1),
        (5, 2),
        (0, 1),
        (1, 2),
        (u128::MAX, 0),
        (u128::MAX, 1),
        (0, u128::MAX),
    ] {
        check(literal(left), literal(right));
    }
}

/// `0 + 7 ≤ x` proves `0 ≤ x − 7` over `u8`: the root's closed sum denotes
/// to its numeral `7`, so the adjunction premise is reached through the
/// interned equation `add 0 7 = 7` in reverse — no instance axiom.
#[test]
fn correlated_subtract_lower_uses_the_add_cancel_law() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let right = literal(7);
    let root = ScalarTerm::exact_integer_add(integer, literal(0), right.clone()).unwrap();
    let premise = Proposition::LessOrEqual(root.clone(), left.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        mathematical(&literal(0)),
        IntegerMathTerm::Subtract(
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
                target: ScalarTerm::exact_integer_subtract(integer, left.clone(), right.clone())
                    .unwrap(),
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
    assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
    assert!(
        elaboration
            .denotation
            .subtraction
            .laws
            .contains_key(&super::Law::CancelAddLeft)
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
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(7),
            IntegerValue::Unsigned(7)
        )
    );
    // A tampered premise or conclusion rejects at the shared relation
    // before denotation — the equation can only be `add 0 7 = 7`.
    let mut wrong_premise = proof.clone();
    let ProofRule::IntegerAffineBound { root_bound, .. } = &mut wrong_premise.rule else {
        unreachable!()
    };
    root_bound.conclusion = Proposition::LessOrEqual(literal(8), left.clone());
    let wrong_assumption = root_bound.conclusion.clone();
    assert!(
        verify_bounded_certificate(
            &context,
            &goal,
            std::slice::from_ref(&wrong_assumption),
            &[],
            &wrong_premise,
            &mut Budget::default()
        )
        .is_err()
    );
    let mut wrong_goal = proof.clone();
    wrong_goal.conclusion = Proposition::IntegerMathLessOrEqual(
        mathematical(&literal(1)),
        IntegerMathTerm::Subtract(
            Box::new(mathematical(&left)),
            Box::new(mathematical(&right)),
        ),
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

/// `−128 + 7 ≤ x` proves `−128 ≤ x − 7` over `i8`: the signed numerals
/// exercise the `negate` prefix through the interned `add (−128) 7 = −121`
/// equation.
#[test]
fn correlated_subtract_signed_lower_uses_the_numeral_equation() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let left = ScalarTerm::value(ValueId::new(1).unwrap(), scalar);
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar)]).unwrap();
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Signed(number)).unwrap();
    let right = literal(7);
    let root = ScalarTerm::exact_integer_add(integer, literal(-128), right.clone()).unwrap();
    let premise = Proposition::LessOrEqual(root.clone(), left.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        mathematical(&literal(-128)),
        IntegerMathTerm::Subtract(
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
                target: ScalarTerm::exact_integer_subtract(integer, left, right).unwrap(),
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
    assert!(
        elaboration
            .denotation
            .subtraction
            .laws
            .contains_key(&super::Law::CancelAddLeft)
    );
    assert_eq!(
        elaboration
            .denotation
            .addition
            .numeral_sums
            .keys()
            .next()
            .unwrap(),
        &(
            IntegerValue::Signed(-128),
            IntegerValue::Signed(7),
            IntegerValue::Signed(-121)
        )
    );
}

/// `x ≤ e + 7` with `e = max` proves `x − 7 ≤ max` over `u64` — the value
/// root substitutes through its cited definition and the open sum reaches
/// the adjunction directly, then the landed endpoint substitutes through
/// its cited literal equality.
#[test]
fn correlated_subtract_upper_through_cited_definitions() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let context = PropositionContext::from_value_types(
        (1..=4).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let endpoint = ScalarTerm::integer(integer, integer.maximum_value()).unwrap();
    let addend = ScalarTerm::integer(integer, IntegerValue::Unsigned(7)).unwrap();
    let expression = ScalarTerm::exact_integer_add(integer, value(4), addend.clone()).unwrap();
    let premise = Proposition::LessOrEqual(value(1), value(3));
    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&addend)),
    );
    let goal = Proposition::IntegerMathLessOrEqual(difference, mathematical(&endpoint));
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
            equality(value(3), expression.clone()),
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
                    target: ScalarTerm::exact_integer_subtract(integer, value(1), addend.clone())
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
        assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
        assert!(
            elaboration
                .denotation
                .subtraction
                .laws
                .contains_key(&super::Law::CancelAddRight)
        );
        // The cited sum is open, so no numeral-operation equation is needed.
        assert!(elaboration.denotation.addition.numeral_sums.is_empty());
    }
}

/// `1 + 7 ≤ 30` proves `0 ≤ 30 − 7` over `u8` — every endpoint is closed,
/// so the numeral laws decide the goal outright and no law chain runs.
#[test]
fn correlated_subtract_closed_goal_decides_by_numeral_laws() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let context = PropositionContext::from_value_types([]).unwrap();
    let left = literal(30);
    let right = literal(7);
    let root = ScalarTerm::exact_integer_add(integer, literal(0), right.clone()).unwrap();
    let premise = Proposition::LessOrEqual(root.clone(), left.clone());
    let goal = Proposition::IntegerMathLessOrEqual(
        mathematical(&literal(0)),
        IntegerMathTerm::Subtract(
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
                target: ScalarTerm::exact_integer_subtract(integer, left, right).unwrap(),
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
    assert!(elaboration.denotation.subtraction.laws.is_empty());
}

#[test]
fn open_mathematical_subtraction_shares_scalar_terms_without_prefix_keys() {
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
        scalar_term = ScalarTerm::exact_integer_subtract(integer, scalar_term, value(2)).unwrap();
        math_term =
            IntegerMathTerm::Subtract(Box::new(math_term), Box::new(mathematical(&value(2))));
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
            eprintln!("open subtraction depth={depth}, new mathematical slots={slots}");
        }
    }
}

#[test]
fn subtraction_identity_transport_accepts_existing_leaf_math_citations() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let compound = ScalarTerm::exact_integer_subtract(integer, value(1), value(2)).unwrap();
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
fn open_subtraction_preserves_previously_skipped_numeric_resource_refusals() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = ValueId::new(1).unwrap();
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
    let large = IntegerMathTerm::ShiftLeft {
        value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
        count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(65_536))),
    };
    let open = IntegerMathTerm::Subtract(
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
