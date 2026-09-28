use super::{
    BTreeMap, BTreeSet, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
    check_certificate, implications, polarity_certificate,
};
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId};
use terminal_psi::{Operation, OperationKind, OperationResult, ValueDeclaration};
use terminal_semantics::{OperationSemanticError, goal_free_scalar_leaf_semantics};

fn polarity_fixture() -> (
    BTreeMap<ValueId, ScalarType>,
    terminal_semantics::GoalFreeScalarLeafSemantics,
    Proposition,
    PropositionContext,
) {
    let types = BTreeMap::from([
        (value(1), ScalarType::Boolean),
        (value(3), ScalarType::Boolean),
    ]);
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanNot { operand: value(1) }),
        &types,
    )
    .unwrap()
    .unwrap();
    let equation = observed.result_equation().clone();
    let mut values = BTreeSet::new();
    equation.visit_value_ids(|value| {
        values.insert(value);
    });
    let context = PropositionContext::from_value_types(
        values
            .iter()
            .filter_map(|value| types.get(value).map(|scalar_type| (*value, *scalar_type))),
    )
    .unwrap();
    (types, observed, equation, context)
}

fn value(index: u64) -> ValueId {
    ValueId::new(index).unwrap()
}

fn term(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(value(index), scalar_type)
}

fn operation(kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: value(3),
            scalar_type: ScalarType::Boolean,
        }),
        kind,
    }
}

#[test]
fn private_crash_projection_keeps_equation_without_call_proof_auxiliaries() {
    use super::super::{OperationFactPurpose, append_scalar_facts};

    let types = BTreeMap::from([
        (value(1), ScalarType::Boolean),
        (value(3), ScalarType::Boolean),
    ]);
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanNot { operand: value(1) }),
        &types,
    )
    .unwrap()
    .unwrap();
    let retained = Proposition::Implication {
        premise: Box::new(Proposition::Truth),
        conclusion: Box::new(Proposition::Equal(
            term(1, ScalarType::Boolean),
            ScalarTerm::boolean(false),
        )),
    };
    let mut crash_facts = vec![retained.clone()];
    append_scalar_facts(
        &observed,
        &types,
        OperationFactPurpose::PrivateCrashPredicates,
        &mut crash_facts,
    )
    .unwrap();
    assert_eq!(
        crash_facts,
        vec![retained.clone(), observed.result_equation().clone()]
    );

    let mut proof_facts = vec![retained];
    append_scalar_facts(
        &observed,
        &types,
        OperationFactPurpose::ProofObligations,
        &mut proof_facts,
    )
    .unwrap();
    assert_eq!(proof_facts[..2], crash_facts);
    assert_eq!(proof_facts[2..], implications(&observed, &types).unwrap());
    assert_eq!(proof_facts.len(), 4);
}

#[test]
fn exact_negation_polarities_have_no_external_premises() {
    let types = BTreeMap::from([
        (value(1), ScalarType::Boolean),
        (value(3), ScalarType::Boolean),
    ]);
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanNot { operand: value(1) }),
        &types,
    )
    .unwrap()
    .unwrap();
    let actual = implications(&observed, &types).unwrap();
    for (index, positive) in [true, false].into_iter().enumerate() {
        assert_eq!(
            actual[index],
            Proposition::Implication {
                premise: Box::new(Proposition::Equal(
                    term(1, ScalarType::Boolean),
                    ScalarTerm::boolean(!positive)
                )),
                conclusion: Box::new(Proposition::Equal(
                    term(3, ScalarType::Boolean),
                    ScalarTerm::boolean(positive)
                )),
            }
        );
    }
    let mut missing_result = types.clone();
    missing_result.remove(&value(3));
    assert!(implications(&observed, &missing_result).is_err());
    let mut retyped_result = types.clone();
    retyped_result.insert(
        value(3),
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap()),
    );
    assert!(implications(&observed, &retyped_result).is_err());
    assert!(matches!(
        goal_free_scalar_leaf_semantics(
            &operation(OperationKind::BooleanNot { operand: value(2) }),
            &types
        ),
        Err(OperationSemanticError::UnknownValue(_))
    ));
}

#[test]
fn integer_comparison_polarities_preserve_exact_strictness_and_operands() {
    let integer = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap());
    let types = BTreeMap::from([
        (value(1), integer),
        (value(2), integer),
        (value(3), ScalarType::Boolean),
    ]);
    for (kind, positive, negative) in [
        (
            OperationKind::IntegerLessThan {
                left: value(1),
                right: value(2),
            },
            Proposition::LessThan(term(1, integer), term(2, integer)),
            Proposition::LessOrEqual(term(2, integer), term(1, integer)),
        ),
        (
            OperationKind::IntegerLessOrEqual {
                left: value(1),
                right: value(2),
            },
            Proposition::LessOrEqual(term(1, integer), term(2, integer)),
            Proposition::LessThan(term(2, integer), term(1, integer)),
        ),
    ] {
        let observed = goal_free_scalar_leaf_semantics(&operation(kind), &types)
            .unwrap()
            .unwrap();
        let actual = implications(&observed, &types).unwrap();
        for (index, premise) in [positive, negative].into_iter().enumerate() {
            assert_eq!(
                actual[index],
                Proposition::Implication {
                    premise: Box::new(premise),
                    conclusion: Box::new(Proposition::Equal(
                        term(3, ScalarType::Boolean),
                        ScalarTerm::boolean(index == 0)
                    )),
                }
            );
        }
    }
}

#[test]
fn boolean_equality_retains_both_operand_polarities() {
    let types = (1..=3)
        .map(|index| (value(index), ScalarType::Boolean))
        .collect();
    let observed = goal_free_scalar_leaf_semantics(
        &operation(OperationKind::BooleanEqual {
            left: value(1),
            right: value(2),
        }),
        &types,
    )
    .unwrap()
    .unwrap();
    let actual = implications(&observed, &types).unwrap();
    assert!(
        matches!(&actual[0], Proposition::Implication { premise, .. }
        if **premise == Proposition::Equal(term(1, ScalarType::Boolean), term(2, ScalarType::Boolean)))
    );
    assert!(
        matches!(&actual[1], Proposition::Implication { premise, .. }
        if matches!(premise.as_ref(), Proposition::Disjunction(parts) if parts.len() == 2))
    );
}

#[test]
fn emitted_implications_are_checked_derivations_of_the_result_equation() {
    let (types, observed, equation, context) = polarity_fixture();
    let Proposition::Equal(_, denotation) = &equation else {
        panic!("scalar leaf retains its result equation")
    };
    for (positive, emitted) in [true, false]
        .into_iter()
        .zip(implications(&observed, &types).unwrap())
    {
        let Proposition::Implication {
            premise,
            conclusion,
        } = &emitted
        else {
            panic!("polarity emission is an implication")
        };
        let denotation_polarity =
            Proposition::Equal(denotation.clone(), ScalarTerm::boolean(positive));
        let certificate = polarity_certificate(
            &equation,
            &denotation_polarity,
            premise,
            conclusion,
            &emitted,
        );
        let acceptance = proof_admission::accept_certificate(
            &context,
            &emitted,
            &[],
            std::slice::from_ref(&equation),
            &certificate,
        )
        .expect("the emitted polarity implication is a checked derivation");
        // The result equation is the only premise the derivation cites; the
        // normalized denotation hypothesis is discharged by implication
        // introduction and never becomes an ambient assumption.
        assert_eq!(
            acceptance.semantic_axioms,
            vec![proof_admission::AcceptedPremise {
                index: 0,
                proposition: equation.clone(),
            }]
        );
        assert!(acceptance.assumptions.is_empty());
    }
}

#[test]
fn mispaired_polarity_implications_fail_their_certificate_check() {
    let (types, observed, equation, context) = polarity_fixture();
    let Proposition::Equal(_, denotation) = &equation else {
        panic!("scalar leaf retains its result equation")
    };
    let emitted = implications(&observed, &types).unwrap();
    let Proposition::Implication {
        premise: positive_premise,
        ..
    } = &emitted[0]
    else {
        panic!("polarity emission is an implication")
    };
    let Proposition::Implication {
        premise: negative_premise,
        ..
    } = &emitted[1]
    else {
        panic!("polarity emission is an implication")
    };
    let truth_polarity = Proposition::Equal(denotation.clone(), ScalarTerm::boolean(true));

    // The falsity premise cannot establish the truth conclusion: the
    // denotation conversion of `x == true` is not `denotation == true`.
    let mispaired = Proposition::Implication {
        premise: negative_premise.clone(),
        conclusion: Box::new(Proposition::Equal(
            term(3, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        )),
    };
    let Proposition::Implication {
        conclusion: mispaired_conclusion,
        ..
    } = &mispaired
    else {
        unreachable!()
    };
    let certificate = polarity_certificate(
        &equation,
        &truth_polarity,
        negative_premise,
        mispaired_conclusion,
        &mispaired,
    );
    assert!(
        check_certificate(
            &context,
            &mispaired,
            &[],
            std::slice::from_ref(&equation),
            &certificate,
        )
        .is_err()
    );

    // The truth premise cannot establish the flipped conclusion: transitivity
    // composes `result == true`, never `result == false`.
    let flipped = Proposition::Implication {
        premise: positive_premise.clone(),
        conclusion: Box::new(Proposition::Equal(
            term(3, ScalarType::Boolean),
            ScalarTerm::boolean(false),
        )),
    };
    let Proposition::Implication {
        conclusion: flipped_conclusion,
        ..
    } = &flipped
    else {
        unreachable!()
    };
    let certificate = polarity_certificate(
        &equation,
        &truth_polarity,
        positive_premise,
        flipped_conclusion,
        &flipped,
    );
    assert!(
        check_certificate(
            &context,
            &flipped,
            &[],
            std::slice::from_ref(&equation),
            &certificate,
        )
        .is_err()
    );
}

#[test]
fn a_polarity_certificate_cannot_establish_another_goal() {
    let (types, observed, equation, context) = polarity_fixture();
    let Proposition::Equal(_, denotation) = &equation else {
        panic!("scalar leaf retains its result equation")
    };
    let emitted = implications(&observed, &types).unwrap();
    let Proposition::Implication {
        premise,
        conclusion,
    } = &emitted[0]
    else {
        panic!("polarity emission is an implication")
    };
    let certificate = polarity_certificate(
        &equation,
        &Proposition::Equal(denotation.clone(), ScalarTerm::boolean(true)),
        premise,
        conclusion,
        &emitted[0],
    );
    // The derivation discharges exactly the emitted implication, not an
    // unrelated goal the caller might substitute.
    assert_eq!(
        check_certificate(
            &context,
            &Proposition::Equal(term(3, ScalarType::Boolean), term(1, ScalarType::Boolean)),
            &[],
            std::slice::from_ref(&equation),
            &certificate,
        ),
        Err(proof_admission::ProofError::CertificateConclusionMismatch)
    );
    // Nor may the same goal be established while citing a different premise
    // roster: the equation axiom is cited by index, so a shifted roster moves
    // the citation.
    let other_axiom = Proposition::Equal(term(1, ScalarType::Boolean), ScalarTerm::boolean(true));
    assert!(
        check_certificate(
            &context,
            &emitted[0],
            &[],
            std::slice::from_ref(&other_axiom),
            &certificate,
        )
        .is_err()
    );
}
