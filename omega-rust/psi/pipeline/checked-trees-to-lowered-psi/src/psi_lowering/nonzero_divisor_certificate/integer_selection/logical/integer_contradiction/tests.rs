use super::*;
use proof_admission::{AcceptedProofRule, accept_certificate, check_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarType, ValueId,
};

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
            let proof = super::super::prove_contradiction(&goal, &assumptions, &axioms);
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
