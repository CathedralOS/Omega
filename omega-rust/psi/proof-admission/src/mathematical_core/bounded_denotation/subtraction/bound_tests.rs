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

/// The `IntegerAffineBound` direct-subtract fixture: a `Conjunction` of
/// the two operand bounds proves `k ≤ sub l r` or `sub l r ≤ k` for the
/// checked endpoint difference `k`. Returns the elaboration's denotation
/// state after the shared checker and kernel have both accepted.
fn direct_subtract_fixture(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &Proposition,
    proof: &ProofNode,
) -> super::super::Denotation {
    verify_bounded_certificate(
        context,
        goal,
        std::slice::from_ref(premise),
        &[],
        proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let assumptions = [premise.clone()];
    let mut elaboration = Elaboration::new(context, goal, &assumptions, &[], &parameters).unwrap();
    elaboration.node(proof).unwrap();
    elaboration.denotation
}

fn direct_subtract_proof(
    integer: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    premise: &Proposition,
    goal: &Proposition,
) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness: IntegerAffineWitness {
                root: left.clone(),
                target: ScalarTerm::exact_integer_subtract(integer, left, right).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    }
}

#[test]
fn direct_subtract_bounds_use_the_antitone_law() {
    // `5 ≤ x ∧ y ≤ 3 ⊢ 2 ≤ x − y` and `x ≤ 50 ∧ 10 ≤ y ⊢ x − y ≤ 40`:
    // the right operand's endpoint lands in the direction opposite the
    // conclusion's — subtraction is antitone on the right.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&value(2))),
    );
    for (lower, left_bound, right_bound, bound) in [
        (
            true,
            Proposition::LessOrEqual(literal(5), value(1)),
            Proposition::LessOrEqual(value(2), literal(3)),
            IntegerValue::Unsigned(2),
        ),
        (
            false,
            Proposition::LessOrEqual(value(1), literal(50)),
            Proposition::LessOrEqual(literal(10), value(2)),
            IntegerValue::Unsigned(40),
        ),
    ] {
        let premise = Proposition::Conjunction(vec![left_bound, right_bound]);
        let bound = IntegerMathTerm::literal(bound);
        let goal = if lower {
            Proposition::IntegerMathLessOrEqual(bound, difference.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(difference.clone(), bound)
        };
        let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
        let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
        assert!(
            denotation.rule_axioms.is_empty(),
            "direct-subtract bounds derive from the antitone monotone law"
        );
        assert_eq!(denotation.subtraction.laws.len(), 1);
        assert!(
            denotation
                .subtraction
                .laws
                .contains_key(&super::Law::MonotoneAntitone)
        );
        assert_eq!(denotation.subtraction.numeral_differences.len(), 1);
        assert!(denotation.carrier_bounds.is_empty());
    }
}

#[test]
fn direct_subtract_bound_truth_operand_uses_carrier_membership() {
    // `Truth(x) ∧ y ≤ 3 ⊢ −3 ≤ x − y` over u8 — the carrier minimum `0`
    // is the open minuend's lower endpoint through the interned
    // membership assumption, and the negative bound stays a canonical
    // math literal; `x ≤ 50 ∧ Truth(y) ⊢ x − y ≤ 50` spends the
    // subtrahend's lower carrier endpoint in the upper direction.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&value(2))),
    );
    // `Truth` on the minuend, `y ≤ 3`: the carrier lower endpoint `0`
    // bounds `x` below, so `0 − 3 ≤ x − y` — a negative math literal.
    let premise = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::LessOrEqual(value(2), literal(3)),
    ]);
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::IntegerLiteral(
            semantic_vocabulary::IntegerMathLiteral::new(true, 3).unwrap(),
        ),
        difference.clone(),
    );
    let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
    let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
    assert!(
        denotation.rule_axioms.is_empty(),
        "the carrier endpoint is a named membership assumption, not an instance axiom"
    );
    assert_eq!(denotation.subtraction.laws.len(), 1);
    assert!(
        denotation
            .subtraction
            .laws
            .contains_key(&super::Law::MonotoneAntitone)
    );
    assert_eq!(
        denotation.carrier_bounds.keys().collect::<Vec<_>>(),
        [&(value(1), true)]
    );
    assert_eq!(
        denotation
            .subtraction
            .numeral_differences
            .keys()
            .collect::<Vec<_>>(),
        [&(
            IntegerValue::Unsigned(0),
            IntegerValue::Unsigned(3),
            IntegerValue::Signed(-3)
        )]
    );
    // `x ≤ 50`, `Truth` on the subtrahend: the carrier lower endpoint
    // `0` is `y`'s lower bound — flipped to the upper direction — so
    // `x − y ≤ 50 − 0`.
    let premise = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(1), literal(50)),
        Proposition::Truth,
    ]);
    let goal = Proposition::IntegerMathLessOrEqual(
        difference,
        IntegerMathTerm::literal(IntegerValue::Unsigned(50)),
    );
    let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
    let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
    assert!(denotation.rule_axioms.is_empty());
    assert_eq!(
        denotation.carrier_bounds.keys().collect::<Vec<_>>(),
        [&(value(2), true)]
    );
}

#[test]
fn direct_subtract_bound_literal_minuend_uses_refl() {
    // `Truth(9) ∧ y ≤ 3 ⊢ 6 ≤ 9 − y` — the literal minuend is its own
    // endpoint through `refl`; the carrier bound is not consulted.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context =
        PropositionContext::from_value_types([(ValueId::new(2).unwrap(), scalar)]).unwrap();
    let premise = Proposition::Conjunction(vec![
        Proposition::Truth,
        Proposition::LessOrEqual(value(2), literal(3)),
    ]);
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Unsigned(6)),
        IntegerMathTerm::Subtract(
            Box::new(mathematical(&literal(9))),
            Box::new(mathematical(&value(2))),
        ),
    );
    let proof = direct_subtract_proof(integer, literal(9), value(2), &premise, &goal);
    let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
    assert!(denotation.rule_axioms.is_empty());
    assert!(denotation.carrier_bounds.is_empty());
    assert_eq!(
        denotation
            .subtraction
            .numeral_differences
            .keys()
            .collect::<Vec<_>>(),
        [&(
            IntegerValue::Unsigned(9),
            IntegerValue::Unsigned(3),
            IntegerValue::Unsigned(6)
        )]
    );
}

#[test]
fn direct_subtract_bound_exact_carrier_boundary_cases() {
    // The checker's `(Exact, Carrier)` boundary cases: `x = MAX ∧ Truth(y)`
    // proves `0 ≤ x − y`, and `x = MIN ∧ Truth(y)` proves `x − y ≤ 0` —
    // both land on the same antitone law with a checked
    // `sub k k = 0` equation.
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&value(2))),
    );
    for (lower, exact, bound) in [
        (true, 255, IntegerValue::Unsigned(0)),
        (false, 0, IntegerValue::Unsigned(0)),
    ] {
        let premise = Proposition::Conjunction(vec![
            Proposition::Equal(value(1), literal(exact)),
            Proposition::Truth,
        ]);
        let bound = IntegerMathTerm::literal(bound);
        let goal = if lower {
            Proposition::IntegerMathLessOrEqual(bound, difference.clone())
        } else {
            Proposition::IntegerMathLessOrEqual(difference.clone(), bound)
        };
        let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
        let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
        assert!(
            denotation.rule_axioms.is_empty(),
            "the boundary case derives through the same antitone law"
        );
        assert_eq!(denotation.subtraction.numeral_differences.len(), 1);
        assert_eq!(
            denotation.carrier_bounds.keys().collect::<Vec<_>>(),
            [&(value(2), !lower)]
        );
    }
}

#[test]
fn direct_subtract_bound_out_of_numeral_range_keeps_instance_fallback() {
    // `x = 0 ∧ y ≤ MAX ⊢ −MAX ≤ x − y` over u128: the checked endpoint
    // difference `-(2^128 − 1)` is a canonical math literal but leaves
    // the representable numeral range, so the certificate keeps the
    // explicit per-rule instance assumption.
    let integer = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=2).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let premise = Proposition::Conjunction(vec![
        Proposition::Equal(value(1), literal(0)),
        Proposition::LessOrEqual(value(2), literal(u128::MAX)),
    ]);
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::IntegerLiteral(
            semantic_vocabulary::IntegerMathLiteral::new(true, u128::MAX).unwrap(),
        ),
        IntegerMathTerm::Subtract(
            Box::new(mathematical(&value(1))),
            Box::new(mathematical(&value(2))),
        ),
    );
    let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
    let denotation = direct_subtract_fixture(&context, &goal, &premise, &proof);
    assert_eq!(
        denotation.rule_axioms.len(),
        1,
        "an unrepresentable endpoint difference keeps the explicit instance assumption"
    );
}

#[test]
fn direct_subtract_bound_rejects_mismatched_certificates() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal =
        |magnitude| ScalarTerm::integer(integer, IntegerValue::Unsigned(magnitude)).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let difference = IntegerMathTerm::Subtract(
        Box::new(mathematical(&value(1))),
        Box::new(mathematical(&value(2))),
    );
    let premise = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(5), value(1)),
        Proposition::LessOrEqual(value(2), literal(3)),
    ]);
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
        difference.clone(),
    );
    let proof = direct_subtract_proof(integer, value(1), value(2), &premise, &goal);
    let assumptions = [premise];
    // A wrong bound literal, a difference over different operands, and a
    // bound on the wrong direction all fail the shared relation check
    // before denotation.
    for wrong_goal in [
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(3)),
            difference.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
            IntegerMathTerm::Subtract(
                Box::new(mathematical(&value(1))),
                Box::new(mathematical(&value(3))),
            ),
        ),
        Proposition::IntegerMathLessOrEqual(
            difference.clone(),
            IntegerMathTerm::literal(IntegerValue::Unsigned(2)),
        ),
    ] {
        let mut invalid = proof.clone();
        invalid.conclusion = wrong_goal.clone();
        assert!(
            verify_bounded_certificate(
                &context,
                &wrong_goal,
                &assumptions,
                &[],
                &invalid,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    // A non-conjunction premise, bounds on the wrong operands, and two
    // same-direction oriented bounds (the right operand must flip).
    for wrong_premise in [
        Proposition::LessOrEqual(literal(5), value(1)),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(5), value(1)),
            Proposition::LessOrEqual(value(1), literal(3)),
        ]),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(literal(5), value(1)),
            Proposition::LessOrEqual(literal(3), value(2)),
        ]),
    ] {
        let mut invalid = proof.clone();
        let ProofRule::IntegerAffineBound { root_bound, .. } = &mut invalid.rule else {
            unreachable!()
        };
        root_bound.conclusion = wrong_premise.clone();
        assert!(
            verify_bounded_certificate(
                &context,
                &goal,
                std::slice::from_ref(&wrong_premise),
                &[],
                &invalid,
                &mut Budget::default()
            )
            .is_err()
        );
    }
    // The witness root must be the minuend.
    let mut invalid = proof.clone();
    let ProofRule::IntegerAffineBound { witness, .. } = &mut invalid.rule else {
        unreachable!()
    };
    witness.root = value(2);
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

/// The `IntegerExactSubtractDefinitionBound` fixture: a minuend bound in the
/// conclusion's direction, a subtrahend bound in the opposite direction, and
/// a cited `out = l - r` definition prove `k ≤ out` or `out ≤ k`.
fn exact_subtract_definition_proof(
    left_premise: &Proposition,
    right_premise: &Proposition,
    goal: &Proposition,
) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerExactSubtractDefinitionBound {
            left_bound: Box::new(ProofNode {
                conclusion: left_premise.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            right_bound: Box::new(ProofNode {
                conclusion: right_premise.clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
            definition_axiom: 0,
        },
    }
}

/// Operand premises, definition and goal for one direction: `lower` pairs
/// the minuend's lower endpoint with the subtrahend's upper endpoint.
struct ExactSubtractCase {
    context: PropositionContext,
    left_premise: Proposition,
    right_premise: Proposition,
    definition: Proposition,
    goal: Proposition,
}

fn exact_subtract_case(
    integer: IntegerType,
    lower: bool,
    reversed: bool,
    endpoints: (IntegerValue, IntegerValue, IntegerValue),
) -> ExactSubtractCase {
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|index| (ValueId::new(index).unwrap(), scalar)),
    )
    .unwrap();
    let (left_endpoint, right_endpoint, bound) = endpoints;
    let literal = |value| ScalarTerm::integer(integer, value).unwrap();
    let output = value(3);
    let expression = ScalarTerm::exact_integer_subtract(integer, value(1), value(2)).unwrap();
    let (left_premise, right_premise) = if lower {
        (
            Proposition::LessOrEqual(literal(left_endpoint), value(1)),
            Proposition::LessOrEqual(value(2), literal(right_endpoint)),
        )
    } else {
        (
            Proposition::LessOrEqual(value(1), literal(left_endpoint)),
            Proposition::LessOrEqual(literal(right_endpoint), value(2)),
        )
    };
    let definition = if reversed {
        Proposition::Equal(expression, output.clone())
    } else {
        Proposition::Equal(output.clone(), expression)
    };
    let goal = if lower {
        Proposition::LessOrEqual(literal(bound), output)
    } else {
        Proposition::LessOrEqual(output, literal(bound))
    };
    ExactSubtractCase {
        context,
        left_premise,
        right_premise,
        definition,
        goal,
    }
}

#[test]
fn exact_subtract_definition_bounds_use_the_two_sided_antitone_law() {
    for (sign, width) in [
        (IntegerSign::Unsigned, 8),
        (IntegerSign::Unsigned, 64),
        (IntegerSign::Signed, 32),
        (IntegerSign::Signed, 128),
    ] {
        let integer = IntegerType::new(sign, width).unwrap();
        for lower in [true, false] {
            let endpoints = match (sign, lower) {
                // `-5 ≤ l ∧ r ≤ 3 ⊢ -8 ≤ l - r`
                (IntegerSign::Signed, true) => (
                    IntegerValue::Signed(-5),
                    IntegerValue::Signed(3),
                    IntegerValue::Signed(-8),
                ),
                // `l ≤ 7 ∧ -9 ≤ r ⊢ l - r ≤ 16`
                (IntegerSign::Signed, false) => (
                    IntegerValue::Signed(7),
                    IntegerValue::Signed(-9),
                    IntegerValue::Signed(16),
                ),
                // `10 ≤ l ∧ r ≤ 4 ⊢ 6 ≤ l - r`
                (IntegerSign::Unsigned, true) => (
                    IntegerValue::Unsigned(10),
                    IntegerValue::Unsigned(4),
                    IntegerValue::Unsigned(6),
                ),
                // `l ≤ 100 ∧ 40 ≤ r ⊢ l - r ≤ 60`
                (IntegerSign::Unsigned, false) => (
                    IntegerValue::Unsigned(100),
                    IntegerValue::Unsigned(40),
                    IntegerValue::Unsigned(60),
                ),
            };
            for reversed in [false, true] {
                let case = exact_subtract_case(integer, lower, reversed, endpoints);
                let proof = exact_subtract_definition_proof(
                    &case.left_premise,
                    &case.right_premise,
                    &case.goal,
                );
                let assumptions = [case.left_premise.clone(), case.right_premise.clone()];
                let axioms = [case.definition.clone()];
                verify_bounded_certificate(
                    &case.context,
                    &case.goal,
                    &assumptions,
                    &axioms,
                    &proof,
                    &mut Budget::default(),
                )
                .unwrap();
                let parameters = BTreeSet::new();
                let mut elaboration = Elaboration::new(
                    &case.context,
                    &case.goal,
                    &assumptions,
                    &axioms,
                    &parameters,
                )
                .unwrap();
                elaboration.node(&proof).unwrap();
                assert!(
                    elaboration.denotation.rule_axioms.is_empty(),
                    "exact-subtract definition bounds derive from the two-sided law"
                );
                assert_eq!(elaboration.denotation.subtraction.laws.len(), 1);
                assert_eq!(
                    elaboration
                        .denotation
                        .subtraction
                        .numeral_differences
                        .keys()
                        .collect::<Vec<_>>(),
                    [&endpoints]
                );
            }
        }
    }
}

/// The subtrahend's bound must point the opposite way from the conclusion,
/// and the landed literal must be the endpoint difference: a same-direction
/// subtrahend bound, a widened literal, or an add definition cited for a
/// subtract rule all reject.
#[test]
fn exact_subtract_definition_bound_rejects_mismatched_certificates() {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let endpoints = (
        IntegerValue::Signed(-5),
        IntegerValue::Signed(3),
        IntegerValue::Signed(-8),
    );
    let case = exact_subtract_case(integer, true, false, endpoints);
    let scalar = ScalarType::Integer(integer);
    let value = |index| ScalarTerm::value(ValueId::new(index).unwrap(), scalar);
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let check =
        |left: &Proposition, right: &Proposition, definition: &Proposition, goal: &Proposition| {
            let proof = exact_subtract_definition_proof(left, right, goal);
            verify_bounded_certificate(
                &case.context,
                goal,
                &[left.clone(), right.clone()],
                std::slice::from_ref(definition),
                &proof,
                &mut Budget::default(),
            )
        };
    assert!(
        check(
            &case.left_premise,
            &case.right_premise,
            &case.definition,
            &case.goal
        )
        .is_ok()
    );
    // The subtrahend's LOWER bound cannot bound the difference from below.
    let same_direction = Proposition::LessOrEqual(literal(3), value(2));
    assert!(
        check(
            &case.left_premise,
            &same_direction,
            &case.definition,
            &case.goal
        )
        .is_err()
    );
    // A literal the endpoints do not reach.
    let widened_goal = Proposition::LessOrEqual(literal(-7), value(3));
    assert!(
        check(
            &case.left_premise,
            &case.right_premise,
            &case.definition,
            &widened_goal
        )
        .is_err()
    );
    // An exact-add definition is not a subtraction.
    let add_definition = Proposition::Equal(
        value(3),
        ScalarTerm::exact_integer_add(integer, value(1), value(2)).unwrap(),
    );
    assert!(
        check(
            &case.left_premise,
            &case.right_premise,
            &add_definition,
            &case.goal
        )
        .is_err()
    );
}
