use super::super::super::super::affine_custody::DefinitionIndex;
use super::{Proposition, ScalarTerm};
use proof_admission::{AcceptedProofRule, ProofNode, accept_certificate, check_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarType, ValueId,
};

fn prove(assumptions: &[Proposition], axioms: &[Proposition]) -> Option<ProofNode> {
    super::prove(
        &context(),
        assumptions,
        axioms,
        &mut DefinitionIndex::new(axioms),
    )
}

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn literal(value: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(value)).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=4).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap()
}

fn relation(left: ScalarTerm, right: ScalarTerm, strict: bool) -> Proposition {
    if strict {
        Proposition::LessThan(left, right)
    } else {
        Proposition::LessOrEqual(left, right)
    }
}

#[test]
fn incompatible_integer_bounds_retain_exact_projected_premises() {
    let goal = Proposition::Equal(value(2), literal(42));
    for (lower, upper) in [(3, 3), (5, 4)] {
        for strict in [[false, false], [true, false], [false, true], [true, true]] {
            let lower_fact = relation(literal(lower), value(1), strict[0]);
            let upper_fact = relation(value(1), literal(upper), strict[1]);
            let assumptions = [
                Proposition::Truth,
                Proposition::Conjunction(vec![Proposition::Truth, lower_fact.clone()]),
            ];
            let axioms = [
                Proposition::Truth,
                Proposition::Conjunction(vec![
                    Proposition::Truth,
                    Proposition::Conjunction(vec![Proposition::Truth, upper_fact.clone()]),
                ]),
            ];
            let proof = super::super::prove_contradiction(
                &context(),
                &goal,
                &assumptions,
                &axioms,
                &mut DefinitionIndex::new(&axioms),
            );
            if lower == upper && !strict[0] && !strict[1] {
                assert!(proof.is_none());
                continue;
            }
            let proof = proof.unwrap();
            let acceptance =
                accept_certificate(&context(), &goal, &assumptions, &axioms, &proof).unwrap();
            assert_eq!(acceptance.assumptions.len(), 1);
            assert_eq!(acceptance.assumptions[0].proposition, assumptions[1]);
            assert_eq!(acceptance.semantic_axioms.len(), 1);
            assert_eq!(acceptance.semantic_axioms[0].proposition, axioms[1]);
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::PredicateDenotation)
            );
            assert!(acceptance.rules.contains(&if strict[0] || strict[1] {
                AcceptedProofRule::IntegerStrictOrderTransitivity
            } else {
                AcceptedProofRule::IntegerLessOrEqualTransitivity
            }));
            for changed in [
                vec![],
                vec![assumptions[1].clone(), Proposition::Truth],
                vec![Proposition::Truth, Proposition::Truth],
                vec![
                    Proposition::Truth,
                    Proposition::Conjunction(vec![lower_fact.clone(), Proposition::Truth]),
                ],
            ] {
                assert!(check_certificate(&context(), &goal, &changed, &axioms, &proof).is_err());
            }
            for changed in [
                vec![],
                vec![axioms[1].clone(), Proposition::Truth],
                vec![
                    Proposition::Truth,
                    Proposition::Conjunction(vec![
                        Proposition::Truth,
                        Proposition::Conjunction(vec![
                            Proposition::Truth,
                            relation(value(1), literal(10), strict[1]),
                        ]),
                    ]),
                ],
            ] {
                assert!(
                    check_certificate(&context(), &goal, &assumptions, &changed, &proof).is_err()
                );
            }
        }
    }
}

#[test]
fn guarded_implication_discharges_only_its_local_integer_contradiction() {
    let lower = Proposition::LessOrEqual(literal(3), value(1));
    let upper = Proposition::LessThan(value(1), literal(3));
    let result = Proposition::Equal(value(2), literal(42));
    let goal = Proposition::Implication {
        premise: Box::new(upper.clone()),
        conclusion: Box::new(result.clone()),
    };
    let axioms = [lower];
    let proof = super::super::super::build(&context(), &goal, &[], &axioms).unwrap();
    let acceptance = accept_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
    assert!(acceptance.assumptions.is_empty());
    assert_eq!(acceptance.semantic_axioms.len(), 1);
    assert!(check_certificate(&context(), &goal, &[], &[], &proof).is_err());
    assert!(super::super::super::build(&context(), &result, &[], &axioms).is_none());
    for conditional in [
        Proposition::Disjunction(vec![upper.clone(), Proposition::Truth]),
        Proposition::Implication {
            premise: Box::new(result.clone()),
            conclusion: Box::new(upper),
        },
    ] {
        assert!(prove(&[conditional], &axioms).is_none());
    }
}

/// A stored bound such as `counter == 9` is an exact equality citation, not an
/// order fact. Weakened to its nonstrict legs it still contradicts a guarded
/// premise like `counter < 3`, in either cited orientation.
#[test]
fn equality_established_integer_bounds_contradict_cited_orders() {
    let upper = Proposition::LessThan(value(1), literal(3));
    for equality in [
        Proposition::Equal(value(1), literal(3)),
        Proposition::Equal(literal(3), value(1)),
    ] {
        let assumptions = [equality.clone()];
        let axioms = [upper.clone()];
        let proof = prove(&assumptions, &axioms).expect("exact bound leg");
        let acceptance = accept_certificate(
            &context(),
            &Proposition::Falsehood,
            &assumptions,
            &axioms,
            &proof,
        )
        .unwrap();
        assert_eq!(acceptance.assumptions.len(), 1);
        assert_eq!(acceptance.semantic_axioms.len(), 1);
        assert!(
            acceptance
                .rules
                .contains(&AcceptedProofRule::IntegerOrderWeakening)
        );
        assert!(
            acceptance
                .rules
                .contains(&AcceptedProofRule::PredicateDenotation)
        );
        for changed in [
            vec![],
            vec![Proposition::Equal(value(1), literal(2))],
            vec![Proposition::Equal(value(2), literal(3))],
        ] {
            assert!(
                check_certificate(
                    &context(),
                    &Proposition::Falsehood,
                    &changed,
                    &axioms,
                    &proof
                )
                .is_err(),
                "the exact bound cannot be removed or redirected"
            );
        }
    }
}

/// Two equalities pinning one value to distinct literals contradict each other
/// through their nonstrict legs; no disequality rule is needed.
#[test]
fn distinct_equality_literals_establish_the_integer_contradiction() {
    let assumptions = [Proposition::Equal(value(1), literal(0))];
    let axioms = [Proposition::Equal(value(1), literal(1))];
    let proof = prove(&assumptions, &axioms).expect("0 <= v <= ... contradiction");
    let acceptance = accept_certificate(
        &context(),
        &Proposition::Falsehood,
        &assumptions,
        &axioms,
        &proof,
    )
    .unwrap();
    assert_eq!(acceptance.assumptions.len(), 1);
    assert_eq!(acceptance.semantic_axioms.len(), 1);
    for changed in [
        vec![],
        vec![Proposition::Equal(value(1), literal(0)), Proposition::Truth],
        vec![Proposition::Equal(value(2), literal(1))],
    ] {
        assert!(
            check_certificate(
                &context(),
                &Proposition::Falsehood,
                &assumptions,
                &changed,
                &proof
            )
            .is_err(),
            "the second equality citation is load-bearing"
        );
    }
}

/// Consistent equalities, non-integer equalities, and mixed-type equalities
/// never weaken into contradictory legs.
#[test]
fn equality_legs_keep_their_exact_integer_custody() {
    let consistent = [
        Proposition::Equal(value(1), literal(3)),
        Proposition::Equal(literal(3), literal(3)),
    ];
    for axioms in [
        vec![Proposition::LessThan(value(1), literal(4))],
        vec![Proposition::Equal(value(1), literal(3))],
        vec![Proposition::LessThan(value(2), literal(3))],
    ] {
        assert!(prove(&consistent, &axioms).is_none(), "{axioms:?}");
    }
    let boolean = Proposition::Equal(
        ScalarTerm::value(ValueId::new(4).unwrap(), ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    assert!(
        prove(&[boolean], &[Proposition::LessThan(value(1), literal(3))]).is_none(),
        "a Boolean equality is not an integer bound"
    );
    let other_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mixed = Proposition::Equal(
        ScalarTerm::value(ValueId::new(4).unwrap(), ScalarType::Integer(other_type)),
        literal(3),
    );
    assert!(
        prove(&[mixed], &[Proposition::LessThan(value(1), literal(3))]).is_none(),
        "a mixed-type equality cannot name the u32 bound"
    );
}

/// A closed arithmetic endpoint evaluates through the checked closed-relation
/// primitive, so `value == 2 + 1` contradicts `value < 3` by literal `3 < 3`.
#[test]
fn closed_arithmetic_endpoints_reach_literal_contradictions() {
    let sum = ScalarTerm::wrapping_integer_add(integer_type(), literal(2), literal(1)).unwrap();
    let assumptions = [Proposition::Equal(value(1), sum)];
    let axioms = [Proposition::LessThan(value(1), literal(3))];
    let proof = prove(&assumptions, &axioms).expect("evaluated bound leg");
    let acceptance = accept_certificate(
        &context(),
        &Proposition::Falsehood,
        &assumptions,
        &axioms,
        &proof,
    )
    .unwrap();
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerOrderSubstitution)
    );
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::PredicateDenotation)
    );
    // Open operands keep their exact identity: no closed denotation exists for
    // `value == v2 + 1` even alongside a strict bound on the same value.
    let open = ScalarTerm::wrapping_integer_add(integer_type(), value(2), literal(1)).unwrap();
    assert!(
        prove(
            &[Proposition::Equal(value(1), open)],
            std::slice::from_ref(&axioms[0])
        )
        .is_none(),
        "an open sum is not a literal bound"
    );
}

/// The guarded-exit shape itself: with `counter == 3` established, the premise
/// `counter < 3` cannot hold, so the implication discharges vacuously while
/// the uncontradicted conclusion alone still does not prove.
#[test]
fn guarded_implication_discharges_an_equality_bound_contradiction() {
    let premise = Proposition::LessThan(value(1), literal(3));
    let result = Proposition::Equal(value(2), literal(42));
    let goal = Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(result.clone()),
    };
    let axioms = [Proposition::Equal(value(1), literal(3))];
    let proof = super::super::super::build(&context(), &goal, &[], &axioms)
        .expect("the equality bound retires the guarded premise");
    let acceptance = accept_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
    assert!(acceptance.assumptions.is_empty());
    assert_eq!(acceptance.semantic_axioms.len(), 1);
    assert!(check_certificate(&context(), &goal, &[], &[], &proof).is_err());
    assert!(
        super::super::super::build(&context(), &result, &[], &axioms).is_none(),
        "the equality bound alone never proves the conclusion"
    );
}

#[test]
fn integer_contradiction_search_rejects_consistent_or_unmatched_bounds() {
    let lower = Proposition::LessOrEqual(literal(3), value(1));
    let other_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    for upper in [
        Proposition::LessThan(value(1), literal(4)),
        Proposition::LessThan(value(2), literal(3)),
        Proposition::LessThan(
            value(1),
            ScalarTerm::integer(other_type, IntegerValue::Unsigned(3)).unwrap(),
        ),
        Proposition::LessThan(
            ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(other_type)),
            literal(3),
        ),
    ] {
        assert!(prove(std::slice::from_ref(&lower), &[upper]).is_none());
    }
    assert!(prove(&[], &[]).is_none());
}

#[test]
fn directly_cited_closed_integer_falsehood_is_not_an_unconditional_primitive() {
    for proposition in [
        Proposition::Equal(literal(3), literal(4)),
        Proposition::LessThan(literal(3), literal(3)),
        Proposition::LessOrEqual(literal(5), literal(4)),
    ] {
        let premises = [proposition];
        let proof = prove(&premises, &[]).unwrap();
        let acceptance =
            accept_certificate(&context(), &Proposition::Falsehood, &premises, &[], &proof)
                .unwrap();
        assert_eq!(acceptance.assumptions.len(), 1);
        assert!(check_certificate(&context(), &Proposition::Falsehood, &[], &[], &proof).is_err());
    }
}

#[test]
fn distinct_integer_bound_subjects_require_the_complete_equality_bridge() {
    let bounds = [Proposition::Conjunction(vec![
        Proposition::LessOrEqual(literal(3), value(1)),
        Proposition::LessThan(value(2), literal(3)),
    ])];
    let equations = [
        Proposition::Equal(value(3), value(1)),
        Proposition::Conjunction(vec![
            Proposition::Truth,
            Proposition::Equal(value(2), value(3)),
        ]),
    ];
    let proof = prove(&bounds, &equations).expect("two independently cited equality edges");
    let acceptance = accept_certificate(
        &context(),
        &Proposition::Falsehood,
        &bounds,
        &equations,
        &proof,
    )
    .unwrap();
    assert_eq!(acceptance.assumptions.len(), 1);
    assert_eq!(acceptance.semantic_axioms.len(), 2);
    for rule in [
        AcceptedProofRule::EqualitySymmetry,
        AcceptedProofRule::EqualityTransitivity,
        AcceptedProofRule::IntegerOrderSubstitution,
        AcceptedProofRule::IntegerStrictOrderTransitivity,
    ] {
        assert!(acceptance.rules.contains(&rule));
    }
    for changed in [
        vec![],
        vec![equations[0].clone()],
        vec![equations[1].clone()],
        vec![equations[1].clone(), equations[0].clone()],
        vec![Proposition::Equal(value(3), value(4)), equations[1].clone()],
        vec![Proposition::Equal(value(1), value(3)), equations[1].clone()],
    ] {
        assert!(
            check_certificate(
                &context(),
                &Proposition::Falsehood,
                &bounds,
                &changed,
                &proof
            )
            .is_err()
        );
    }
    for missing in 0..equations.len() {
        let mut changed = equations.to_vec();
        changed[missing] = Proposition::Truth;
        assert!(prove(&bounds, &changed).is_none());
    }
    let redirected = [Proposition::Equal(value(3), value(4)), equations[1].clone()];
    assert!(prove(&bounds, &redirected).is_none());
    for conditional in [
        Proposition::Disjunction(vec![equations[0].clone(), Proposition::Truth]),
        Proposition::Implication {
            premise: Box::new(Proposition::Equal(value(4), literal(42))),
            conclusion: Box::new(equations[0].clone()),
        },
    ] {
        assert!(prove(&bounds, &[conditional, equations[1].clone()]).is_none());
    }
}
