use crate::proof::{assumption_proof, semantic_axiom_proof};
use semantic_vocabulary::{IntegerMathTerm, IntegerValue, ScalarTerm};

use super::{
    AcceptedPremise, AcceptedProofRule, BTreeSet, IntegerAffineBoundConversionError,
    IntegerAffineWitnessError, IntegerCastBoundConversionError, IntegerCastChainWitnessError,
    IntegerCorrelatedForbiddenRootConversionError, IntegerCorrelatedForbiddenRootWitnessError,
    ProofError, ProofNode, ProofRule, Proposition, PropositionContext, ValueId, accept_certificate,
    accept_certificate_with_machine_parameters, check_certificate, lift_fixed_integer_relation,
};
use crate::IntegerAffineWitness;
use crate::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment,
};
use semantic_vocabulary::PropositionId;

fn transport_certificate() -> (PropositionContext, Vec<Proposition>, ProofNode) {
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let scalar_type = ScalarType::Integer(integer);
    let source = ValueId::new(1).expect("source");
    let destination = ValueId::new(2).expect("destination");
    let context =
        PropositionContext::from_value_types([(source, scalar_type), (destination, scalar_type)])
            .expect("context");
    let source = ScalarTerm::value(source, scalar_type);
    let destination = ScalarTerm::value(destination, scalar_type);
    let ceiling = ScalarTerm::integer(integer, IntegerValue::Signed(100)).expect("ceiling");
    let axioms = vec![
        Proposition::LessOrEqual(source.clone(), ceiling.clone()),
        Proposition::Equal(destination.clone(), source),
    ];
    let proof = ProofNode {
        conclusion: Proposition::LessOrEqual(destination, ceiling),
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
            equalities: vec![semantic_axiom_proof(axioms[1].clone(), 1)],
        },
    };
    (context, axioms, proof)
}

#[test]
fn certificate_receipt_reuses_admission_verification_stack() {
    super::run_on_verification_stack(|| {
        let (context, axioms, proof) = transport_certificate();
        let before = crate::mathematical_core::verification_workers_started();
        let accepted = super::accept_certificate_on_current_thread(
            &context,
            &proof.conclusion,
            &[],
            &axioms,
            &BTreeSet::new(),
            &proof,
        )
        .expect("transport certificate");
        assert_eq!(accepted.semantic_axioms.len(), 2);
        assert!(matches!(
            accepted.mathematical_core,
            super::MathematicalCoreDecision::Judged(_)
        ));
        assert_eq!(
            crate::mathematical_core::verification_workers_started(),
            before,
            "receipt traversal already owns the admission stack"
        );
    });
}

#[test]
fn standalone_certificate_receipt_reserves_stack_and_matches_admission() {
    let (context, axioms, proof) = transport_certificate();
    let accepted = accept_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
        .expect("transport certificate");
    let denoted = crate::verify_bounded_certificate(
        &context,
        &proof.conclusion,
        &[],
        &axioms,
        &proof,
        &mut crate::Budget::default(),
    )
    .expect("independent denotation and kernel check");
    let before = crate::mathematical_core::verification_workers_started();
    let receipt = denoted.receipt();
    assert_eq!(
        crate::mathematical_core::verification_workers_started(),
        before + 1
    );
    assert_eq!(
        accepted.mathematical_core,
        super::MathematicalCoreDecision::Judged(receipt)
    );
}

#[test]
#[ignore = "manual certificate admission timing; cargo test -- --ignored --nocapture"]
fn measure_repeated_transport_certificate_admission() {
    let (context, axioms, proof) = transport_certificate();
    let started = std::time::Instant::now();
    for _ in 0..1000 {
        let accepted = accept_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
            .expect("transport certificate");
        std::hint::black_box(accepted);
    }
    eprintln!("1000 transport admissions: {:?}", started.elapsed());
}

#[test]
fn existing_assumption_rule_checks_fixed_carrier_normalization_to_math() {
    let integer_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 16)
            .expect("i16");
    let value_id = semantic_vocabulary::ValueId::new(88).expect("value");
    let value = ScalarTerm::value(
        value_id,
        semantic_vocabulary::ScalarType::Integer(integer_type),
    );
    let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero");
    let assumption = Proposition::LessOrEqual(zero, value);
    let goal = lift_fixed_integer_relation(&assumption).expect("mathematical carrier relation");
    let context = PropositionContext::from_value_types([(
        value_id,
        semantic_vocabulary::ScalarType::Integer(integer_type),
    )])
    .expect("context");
    let proof = assumption_proof(goal.clone(), 0);
    assert!(check_certificate(&context, &goal, &[assumption], &[], &proof).is_ok());

    let wrong = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Signed(1)),
        IntegerMathTerm::MathValue {
            source_type: integer_type,
            value: value_id,
        },
    );
    let tampered = assumption_proof(wrong.clone(), 0);
    assert!(
        check_certificate(
            &context,
            &wrong,
            &[Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero"),
                ScalarTerm::value(
                    value_id,
                    semantic_vocabulary::ScalarType::Integer(integer_type)
                ),
            )],
            &[],
            &tampered
        )
        .is_err()
    );
}

#[test]
fn disjunction_introduction_checks_one_exact_selected_child() {
    let left = Proposition::Atom(PropositionId::new(1).expect("left atom"));
    let right = Proposition::Atom(PropositionId::new(2).expect("right atom"));
    let goal = Proposition::Disjunction(vec![left.clone(), right.clone()]);
    let branch = |proposition: Proposition, assumption: usize, index: usize| ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(assumption_proof(proposition, assumption)),
            index,
        },
    };
    let assumptions = vec![left.clone(), right.clone()];

    for (proof, expected) in [
        (branch(left.clone(), 0, 0), left.clone()),
        (branch(right.clone(), 1, 1), right.clone()),
    ] {
        let accepted = accept_certificate(
            &PropositionContext::default(),
            &goal,
            &assumptions,
            &[],
            &proof,
        )
        .expect("selected disjunct is independently established");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::DisjunctionIntroduction,
            ]
        );
        assert_eq!(
            accepted.assumptions,
            vec![AcceptedPremise {
                index: usize::from(expected == right),
                proposition: expected,
            }]
        );
    }

    assert_eq!(
        check_certificate(
            &PropositionContext::default(),
            &goal,
            &assumptions,
            &[],
            &branch(left.clone(), 0, 2),
        ),
        Err(ProofError::UnknownDisjunct(2))
    );
    assert_eq!(
        check_certificate(
            &PropositionContext::default(),
            &goal,
            &assumptions,
            &[],
            &branch(left.clone(), 0, 1),
        ),
        Err(ProofError::DisjunctConclusionMismatch)
    );
    let non_disjunction = Proposition::Implication {
        premise: Box::new(left.clone()),
        conclusion: Box::new(left.clone()),
    };
    assert_eq!(
        check_certificate(
            &PropositionContext::default(),
            &non_disjunction,
            &assumptions,
            &[],
            &ProofNode {
                conclusion: non_disjunction.clone(),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(assumption_proof(left, 0)),
                    index: 0,
                },
            },
        ),
        Err(ProofError::RuleConclusionMismatch(
            "disjunction introduction"
        ))
    );
}

#[test]
fn integer_order_transitivity_weakens_a_negative_bound_for_nonzero() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = ScalarTerm::value(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    );
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let negative_two_bound = Proposition::LessOrEqual(divisor.clone(), literal(-2));
    let negative_one_bound = Proposition::LessOrEqual(divisor.clone(), literal(-1));
    let positive_bound = Proposition::LessOrEqual(literal(1), divisor.clone());
    let goal = Proposition::Disjunction(vec![negative_one_bound.clone(), positive_bound]);
    let weakened = ProofNode {
        conclusion: negative_one_bound,
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(semantic_axiom_proof(
                negative_two_bound.clone(),
                0,
            )),
            middle_less_or_equal_right: Box::new(ProofNode {
                conclusion: Proposition::LessOrEqual(literal(-2), literal(-1)),
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
        },
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(weakened),
            index: 0,
        },
    };
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    )])
    .expect("context");
    let acceptance = accept_certificate(
        &context,
        &goal,
        &[],
        std::slice::from_ref(&negative_two_bound),
        &proof,
    )
    .expect("the tighter negative bound proves the canonical negative arm");
    assert_eq!(
        acceptance.rules,
        vec![
            AcceptedProofRule::Primitive,
            AcceptedProofRule::SemanticAxiom,
            AcceptedProofRule::DisjunctionIntroduction,
            AcceptedProofRule::IntegerLessOrEqualTransitivity,
        ]
    );
    assert_eq!(
        acceptance.semantic_axioms,
        vec![AcceptedPremise {
            index: 0,
            proposition: negative_two_bound,
        }]
    );
}

#[test]
fn integer_order_transitivity_requires_exact_middle_endpoints_and_relations() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::value(ValueId::new(1).expect("left"), ScalarType::Integer(integer));
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let first = Proposition::LessOrEqual(left.clone(), literal(-2));
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).expect("left"),
        ScalarType::Integer(integer),
    )])
    .expect("context");
    let proof = |second: Proposition, conclusion: Proposition| ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(assumption_proof(first.clone(), 0)),
            middle_less_or_equal_right: Box::new(ProofNode {
                conclusion: second,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
        },
    };

    let expected = Proposition::LessOrEqual(left.clone(), literal(-1));
    assert_eq!(
        check_certificate(
            &context,
            &expected,
            std::slice::from_ref(&first),
            &[],
            &proof(
                Proposition::LessOrEqual(literal(-3), literal(-1)),
                expected.clone(),
            ),
        ),
        Err(ProofError::IntegerOrderMiddleMismatch),
    );

    let wider = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let wider_literal =
        |value| ScalarTerm::integer(wider, IntegerValue::Signed(value)).expect("i16 literal");
    assert_eq!(
        check_certificate(
            &context,
            &expected,
            std::slice::from_ref(&first),
            &[],
            &proof(
                Proposition::LessOrEqual(wider_literal(-2), wider_literal(-1)),
                expected.clone(),
            ),
        ),
        Err(ProofError::IntegerOrderMiddleMismatch),
    );

    let wrong_conclusion = Proposition::LessOrEqual(left, literal(0));
    assert_eq!(
        check_certificate(
            &context,
            &wrong_conclusion,
            std::slice::from_ref(&first),
            &[],
            &proof(
                Proposition::LessOrEqual(literal(-2), literal(-1)),
                wrong_conclusion.clone(),
            ),
        ),
        Err(ProofError::IntegerOrderConclusionMismatch),
    );

    assert_eq!(
        check_certificate(
            &context,
            &expected,
            std::slice::from_ref(&first),
            &[],
            &proof(
                Proposition::Equal(literal(-2), literal(-2)),
                expected.clone(),
            ),
        ),
        Err(ProofError::RulePremiseMismatch("integer <= transitivity")),
    );
}

#[test]
fn integer_order_substitution_transports_either_endpoint_in_either_equality_orientation() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = ScalarTerm::value(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    );
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    )])
    .expect("context");

    let positive_literal = Proposition::LessOrEqual(literal(1), literal(5));
    let positive_equality = Proposition::Equal(divisor.clone(), literal(5));
    let positive = Proposition::LessOrEqual(literal(1), divisor.clone());
    let positive_proof = ProofNode {
        conclusion: positive.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(ProofNode {
                conclusion: positive_literal,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(semantic_axiom_proof(positive_equality.clone(), 0)),
            endpoint: 1,
        },
    };
    let accepted = accept_certificate(
        &context,
        &positive,
        &[],
        std::slice::from_ref(&positive_equality),
        &positive_proof,
    )
    .expect("a literal equality transports the right endpoint");
    assert_eq!(
        accepted.rules,
        vec![
            AcceptedProofRule::Primitive,
            AcceptedProofRule::SemanticAxiom,
            AcceptedProofRule::IntegerOrderSubstitution,
        ]
    );
    assert_eq!(
        accepted.semantic_axioms,
        vec![AcceptedPremise {
            index: 0,
            proposition: positive_equality,
        }]
    );

    let negative_literal = Proposition::LessOrEqual(literal(-2), literal(-1));
    let reverse_negative_equality = Proposition::Equal(literal(-2), divisor.clone());
    let negative = Proposition::LessOrEqual(divisor, literal(-1));
    let negative_proof = ProofNode {
        conclusion: negative.clone(),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(ProofNode {
                conclusion: negative_literal,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(assumption_proof(reverse_negative_equality.clone(), 0)),
            endpoint: 0,
        },
    };
    let accepted = accept_certificate(
        &context,
        &negative,
        std::slice::from_ref(&reverse_negative_equality),
        &[],
        &negative_proof,
    )
    .expect("a reverse equality transports the left endpoint");
    assert_eq!(
        accepted.rules,
        vec![
            AcceptedProofRule::Primitive,
            AcceptedProofRule::Assumption,
            AcceptedProofRule::IntegerOrderSubstitution,
        ]
    );
}

#[test]
fn integer_order_substitution_rejects_every_shape_and_endpoint_mismatch() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = ScalarTerm::value(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    );
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).expect("divisor"),
        ScalarType::Integer(integer),
    )])
    .expect("context");
    let relation = Proposition::LessOrEqual(literal(1), literal(5));
    let equality = Proposition::Equal(literal(5), divisor.clone());
    let conclusion = Proposition::LessOrEqual(literal(1), divisor.clone());
    let child = |conclusion: Proposition, rule: ProofRule| ProofNode { conclusion, rule };
    let proof = |relation: ProofNode,
                 equality: ProofNode,
                 endpoint: usize,
                 conclusion: Proposition| ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(relation),
            equality: Box::new(equality),
            endpoint,
        },
    };
    let relation_child = || {
        child(
            relation.clone(),
            ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        )
    };
    let equality_child = || child(equality.clone(), ProofRule::SemanticAxiom { index: 0 });
    let check = |proof: &ProofNode, goal: &Proposition, axioms: &[Proposition]| {
        check_certificate(&context, goal, &[], axioms, proof)
    };

    assert_eq!(
        check(
            &proof(relation_child(), equality_child(), 2, conclusion.clone()),
            &conclusion,
            std::slice::from_ref(&equality),
        ),
        Err(ProofError::UnknownIntegerOrderEndpoint(2)),
    );

    let changed_other_endpoint = Proposition::LessOrEqual(literal(0), divisor.clone());
    assert_eq!(
        check(
            &proof(
                relation_child(),
                equality_child(),
                1,
                changed_other_endpoint.clone(),
            ),
            &changed_other_endpoint,
            std::slice::from_ref(&equality),
        ),
        Err(ProofError::IntegerOrderUnchangedEndpointMismatch),
    );

    let unrelated = Proposition::Equal(literal(4), divisor);
    assert_eq!(
        check(
            &proof(
                relation_child(),
                child(unrelated.clone(), ProofRule::SemanticAxiom { index: 0 },),
                1,
                conclusion.clone(),
            ),
            &conclusion,
            std::slice::from_ref(&unrelated),
        ),
        Err(ProofError::IntegerOrderSubstitutionMismatch),
    );

    let truth = child(
        Proposition::Truth,
        ProofRule::Primitive(PrimitiveJudgment::Truth),
    );
    assert_eq!(
        check(
            &proof(truth.clone(), equality_child(), 1, conclusion.clone()),
            &conclusion,
            std::slice::from_ref(&equality),
        ),
        Err(ProofError::RulePremiseMismatch(
            "integer order substitution relation"
        )),
    );
    assert_eq!(
        check(
            &proof(relation_child(), truth.clone(), 1, conclusion.clone()),
            &conclusion,
            &[],
        ),
        Err(ProofError::RulePremiseMismatch(
            "integer order substitution equality"
        )),
    );
    assert_eq!(
        check(
            &proof(relation_child(), equality_child(), 1, Proposition::Truth),
            &Proposition::Truth,
            std::slice::from_ref(&equality),
        ),
        Err(ProofError::RuleConclusionMismatch(
            "integer order substitution"
        )),
    );
    assert_eq!(
        check(
            &proof(
                child(relation, ProofRule::SemanticAxiom { index: 1 }),
                equality_child(),
                1,
                conclusion.clone(),
            ),
            &conclusion,
            std::slice::from_ref(&equality),
        ),
        Err(ProofError::UnknownSemanticAxiom(1)),
    );
}

#[test]
fn integer_affine_bound_checks_root_proof_normalization_and_exact_custody() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(integer));
    let target = ScalarTerm::value(
        ValueId::new(2).expect("target"),
        ScalarType::Integer(integer),
    );
    let sibling = ScalarTerm::value(
        ValueId::new(3).expect("sibling"),
        ScalarType::Integer(integer),
    );
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).expect("root"), ScalarType::Integer(integer)),
        (
            ValueId::new(2).expect("target"),
            ScalarType::Integer(integer),
        ),
        (
            ValueId::new(3).expect("sibling"),
            ScalarType::Integer(integer),
        ),
    ])
    .expect("context");
    let root_bound = Proposition::LessOrEqual(literal(1), root.clone());
    let landing = Proposition::Equal(sibling.clone(), literal(2));
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_add(integer, root.clone(), sibling).expect("exact add"),
    );
    let semantic_axioms = [landing.clone(), definition.clone()];
    let conclusion = Proposition::LessOrEqual(literal(3), target.clone());
    let proof =
        |definition_axioms: Vec<usize>, literal_axioms, conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                witness: IntegerAffineWitness {
                    root: root.clone(),
                    target: target.clone(),
                    literal_axioms,
                    definition_axioms,
                },
            },
        };

    let accepted = accept_certificate(
        &context,
        &conclusion,
        std::slice::from_ref(&root_bound),
        &semantic_axioms,
        &proof(vec![1], vec![Some(0)], conclusion.clone()),
    )
    .expect("root proof and exact affine definition prove the mapped bound");
    assert_eq!(
        accepted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::IntegerAffineBound,
        ]
    );
    assert_eq!(
        accepted.semantic_axioms,
        vec![
            AcceptedPremise {
                index: 0,
                proposition: landing,
            },
            AcceptedPremise {
                index: 1,
                proposition: definition.clone(),
            },
        ]
    );
    assert_eq!(
        check_certificate(
            &context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            &semantic_axioms,
            &proof(vec![2], vec![Some(0)], conclusion.clone()),
        ),
        Err(ProofError::IntegerAffineWitness(
            IntegerAffineWitnessError::UnknownSemanticAxiom(2),
        )),
    );
    let wrong_conclusion = Proposition::LessOrEqual(literal(2), target.clone());
    assert_eq!(
        check_certificate(
            &context,
            &wrong_conclusion,
            std::slice::from_ref(&root_bound),
            &semantic_axioms,
            &proof(vec![1], vec![Some(0)], wrong_conclusion.clone()),
        ),
        Err(ProofError::IntegerAffineBoundConversion(
            IntegerAffineBoundConversionError::ConclusionMismatch,
        )),
    );
    let non_order_proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            }),
            witness: IntegerAffineWitness {
                root,
                target,
                definition_axioms: vec![1],
                literal_axioms: vec![Some(0)],
            },
        },
    };
    assert_eq!(
        check_certificate(
            &context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            &semantic_axioms,
            &non_order_proof,
        ),
        Err(ProofError::IntegerAffineBoundConversion(
            IntegerAffineBoundConversionError::TruthRootWithoutTotalImage,
        )),
    );
}

#[test]
fn integer_cast_bound_checks_complete_definition_word_and_exact_custody() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = ScalarTerm::value(
        ValueId::new(1).expect("root"),
        ScalarType::Integer(i16_type),
    );
    let target = ScalarTerm::value(
        ValueId::new(2).expect("target"),
        ScalarType::Integer(i8_type),
    );
    let middle = ScalarTerm::value(
        ValueId::new(3).expect("middle"),
        ScalarType::Integer(u16_type),
    );
    let context = PropositionContext::from_value_types([
        (
            ValueId::new(1).expect("root"),
            ScalarType::Integer(i16_type),
        ),
        (
            ValueId::new(2).expect("target"),
            ScalarType::Integer(i8_type),
        ),
        (
            ValueId::new(3).expect("middle"),
            ScalarType::Integer(u16_type),
        ),
    ])
    .expect("context");
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("i16 one"),
        root.clone(),
    );
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_exact_cast(i16_type, i8_type, root.clone()).expect("partial cast"),
    );
    let conclusion = Proposition::LessOrEqual(
        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("i8 one"),
        target.clone(),
    );
    let proof = |definition_axioms, conclusion: Proposition| ProofNode {
        conclusion,
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
            witness: IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            },
        },
    };

    let accepted = accept_certificate(
        &context,
        &conclusion,
        std::slice::from_ref(&root_bound),
        std::slice::from_ref(&definition),
        &proof(vec![0], conclusion.clone()),
    )
    .expect("one cast maps the independently proved root bound");
    assert_eq!(
        accepted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::IntegerCastBound,
        ]
    );
    assert_eq!(
        accepted.semantic_axioms,
        vec![AcceptedPremise {
            index: 0,
            proposition: definition.clone(),
        }]
    );
    let first_definition = Proposition::Equal(
        middle.clone(),
        ScalarTerm::integer_exact_cast(i16_type, u16_type, root.clone())
            .expect("first partial cast"),
    );
    let second_definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_exact_cast(u16_type, i8_type, middle).expect("second partial cast"),
    );
    let multi_axioms = [first_definition.clone(), second_definition.clone()];
    let accepted = accept_certificate(
        &context,
        &conclusion,
        std::slice::from_ref(&root_bound),
        &multi_axioms,
        &proof(vec![0, 1], conclusion.clone()),
    )
    .expect("the complete contiguous cast word maps the root bound");
    assert_eq!(
        accepted.semantic_axioms,
        vec![
            AcceptedPremise {
                index: 0,
                proposition: first_definition,
            },
            AcceptedPremise {
                index: 1,
                proposition: second_definition,
            },
        ],
    );
    assert_eq!(
        check_certificate(
            &context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
            &proof(vec![1], conclusion.clone()),
        ),
        Err(ProofError::IntegerCastChainWitness(
            IntegerCastChainWitnessError::UnknownSemanticAxiom(1),
        )),
    );
    let wrong_conclusion = Proposition::LessOrEqual(
        target.clone(),
        ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("i8 one"),
    );
    assert_eq!(
        check_certificate(
            &context,
            &wrong_conclusion,
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
            &proof(vec![0], wrong_conclusion.clone()),
        ),
        Err(ProofError::IntegerCastBoundConversion(
            IntegerCastBoundConversionError::ConclusionTargetMismatch,
        )),
    );
}

#[test]
fn implication_certificate_is_checked_structurally() {
    let proposition = Proposition::Atom(PropositionId::new(1).expect("atom identity"));
    let goal = Proposition::Implication {
        premise: Box::new(proposition.clone()),
        conclusion: Box::new(proposition.clone()),
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(assumption_proof(proposition, 0)),
        },
    };
    let accepted = accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("P implies P");
    assert_eq!(
        accepted.rules,
        vec![
            AcceptedProofRule::Assumption,
            AcceptedProofRule::ImplicationIntroduction,
        ]
    );
    assert!(accepted.assumptions.is_empty());
}

#[test]
fn accepted_trust_closure_binds_the_exact_cited_premise() {
    let cited = Proposition::Atom(PropositionId::new(1).expect("cited atom"));
    let replacement = Proposition::Atom(PropositionId::new(2).expect("replacement atom"));
    let proof = assumption_proof(cited.clone(), 0);
    let accepted = accept_certificate(
        &PropositionContext::default(),
        &cited,
        std::slice::from_ref(&cited),
        &[],
        &proof,
    )
    .expect("exact cited premise");
    assert_eq!(
        accepted.assumptions,
        vec![AcceptedPremise {
            index: 0,
            proposition: cited.clone(),
        }]
    );
    assert_eq!(
        accept_certificate(
            &PropositionContext::default(),
            &cited,
            &[replacement],
            &[],
            &proof,
        ),
        Err(ProofError::AssumptionConclusionMismatch(0))
    );
}

#[test]
fn nested_scopes_do_not_export_discharged_same_index_premises() {
    let first = Proposition::Atom(PropositionId::new(1).expect("first atom"));
    let second = Proposition::Atom(PropositionId::new(2).expect("second atom"));
    let implication = |proposition: &Proposition| Proposition::Implication {
        premise: Box::new(proposition.clone()),
        conclusion: Box::new(proposition.clone()),
    };
    let branch = |proposition: &Proposition| ProofNode {
        conclusion: implication(proposition),
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(assumption_proof(proposition.clone(), 0)),
        },
    };
    let goal = Proposition::Conjunction(vec![implication(&first), implication(&second)]);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(vec![branch(&first), branch(&second)]),
    };
    let accepted = accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("both nested implication premises");
    assert!(accepted.assumptions.is_empty());
}

#[test]
fn semantic_equalities_compose_only_through_the_same_middle_term() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
    };

    let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let a = ScalarTerm::value(ValueId::new(1).expect("a"), ScalarType::Integer(integer));
    let b = ScalarTerm::value(ValueId::new(2).expect("b"), ScalarType::Integer(integer));
    let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
    let axioms = vec![
        Proposition::Equal(a.clone(), b.clone()),
        Proposition::Equal(b.clone(), seven.clone()),
    ];
    let goal = Proposition::Equal(a.clone(), seven.clone());
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
            middle_equals_right: Box::new(semantic_axiom_proof(axioms[1].clone(), 1)),
        },
    };
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).expect("a"), ScalarType::Integer(integer)),
        (ValueId::new(2).expect("b"), ScalarType::Integer(integer)),
    ])
    .expect("context");
    let accepted =
        accept_certificate(&context, &goal, &[], &axioms, &proof).expect("transitive equality");
    assert_eq!(
        accepted.semantic_axioms,
        vec![
            AcceptedPremise {
                index: 0,
                proposition: axioms[0].clone(),
            },
            AcceptedPremise {
                index: 1,
                proposition: axioms[1].clone(),
            },
        ]
    );
}

#[test]
fn canonical_content_equalities_compose_through_either_orientation() {
    use semantic_vocabulary::{
        ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
        ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
        ContentStructuralPlace, ContentTerm, PlaceId, StructuralPlaceKind,
    };

    let root = PlaceId::new(1).expect("place");
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(2).expect("domain"),
        projection_report_fingerprint: 3,
    };
    let term = |field: &str| ContentTerm::Projection {
        projection,
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Current,
            root,
            segments: vec![ContentPlaceSegment::Field(field.to_owned())],
        },
    };
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::CountedQuantity,
        parameter: "Byte".to_owned(),
    };
    let a = term("a");
    let b = term("b");
    let c = term("c");
    let axioms = vec![
        Proposition::ContentConservation(ContentConservation::new(
            algebra.clone(),
            a.clone(),
            c.clone(),
        )),
        Proposition::ContentConservation(ContentConservation::new(algebra.clone(), b.clone(), c)),
    ];
    let goal = Proposition::ContentConservation(ContentConservation::new(algebra, a, b));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
            middle_equals_right: Box::new(semantic_axiom_proof(axioms[1].clone(), 1)),
        },
    };
    let context = PropositionContext::from_value_types_and_places(
        [],
        [(root, StructuralPlaceKind::Parameter { position: 0 })],
    )
    .expect("context");

    check_certificate(&context, &goal, &[], &axioms, &proof)
        .expect("canonical equality orientation must not erase transitivity");
}

fn correlated_division_fixture() -> (
    PropositionContext,
    Vec<Proposition>,
    Vec<Proposition>,
    BTreeSet<ValueId>,
    ProofNode,
) {
    let integer_type =
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 8)
            .expect("signed carrier");
    let scalar_type = semantic_vocabulary::ScalarType::Integer(integer_type);
    let value = |raw| ScalarTerm::value(ValueId::new(raw).expect("value"), scalar_type);
    let integer =
        |raw| ScalarTerm::integer(integer_type, IntegerValue::Signed(raw)).expect("literal");
    let root = value(100);
    let dividend = value(101);
    let product = value(102);
    let divisor = value(103);
    let semantic_axioms = vec![
        Proposition::Equal(
            dividend.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), integer(-2))
                .expect("multiply"),
        ),
        Proposition::Equal(
            product.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, root.clone(), integer(2))
                .expect("multiply"),
        ),
        Proposition::Equal(
            divisor.clone(),
            ScalarTerm::exact_integer_add(integer_type, product, integer(1)).expect("add"),
        ),
    ];
    let requirements = vec![
        Proposition::LessOrEqual(integer(-1), root.clone()),
        Proposition::LessOrEqual(root.clone(), integer(0)),
    ];
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor.clone(), integer(-2)),
        Proposition::LessOrEqual(integer(1), divisor.clone()),
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(-1)),
            Proposition::LessOrEqual(integer(-127), dividend.clone()),
        ]),
    ]);
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::IntegerCorrelatedForbiddenRoots {
            witness: IntegerCorrelatedForbiddenRootWitness {
                dividend: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: dividend,
                    steps: vec![CorrelatedAffineStepWitness {
                        definition_axiom: 0,
                        literal_axiom: None,
                    }],
                },
                divisor: CorrelatedAffineBranchWitness {
                    root: root.clone(),
                    target: divisor,
                    steps: vec![
                        CorrelatedAffineStepWitness {
                            definition_axiom: 1,
                            literal_axiom: None,
                        },
                        CorrelatedAffineStepWitness {
                            definition_axiom: 2,
                            literal_axiom: None,
                        },
                    ],
                },
                definition_axiom_count: 3,
                lower_bound_axiom: 3,
                upper_bound_axiom: 4,
                conclusion: Proposition::Conjunction(requirements.clone()),
            },
        },
    };
    let context = PropositionContext::from_value_types((100..=103).map(|raw| {
        (
            ValueId::new(raw).expect("value"),
            semantic_vocabulary::ScalarType::Integer(integer_type),
        )
    }))
    .expect("context");
    (
        context,
        semantic_axioms,
        requirements,
        BTreeSet::from([ValueId::new(100).expect("root")]),
        proof,
    )
}

#[test]
fn correlated_forbidden_root_rule_binds_parameter_and_exact_premise_closure() {
    let (context, axioms, requirements, parameters, proof) = correlated_division_fixture();
    let accepted = accept_certificate_with_machine_parameters(
        &context,
        &proof.conclusion,
        &requirements,
        &axioms,
        &parameters,
        &proof,
    )
    .expect("safe same-parameter correlated division");

    assert_eq!(
        accepted.rules,
        vec![AcceptedProofRule::IntegerCorrelatedForbiddenRoots]
    );
    assert_eq!(
        accepted
            .semantic_axioms
            .iter()
            .map(|premise| premise.index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        accepted
            .assumptions
            .iter()
            .map(|premise| premise.index)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
}

#[test]
fn correlated_forbidden_root_rule_rejects_nonparameter_root_and_forged_conclusion() {
    let (context, axioms, requirements, _, proof) = correlated_division_fixture();
    assert!(matches!(
        accept_certificate_with_machine_parameters(
            &context,
            &proof.conclusion,
            &requirements,
            &axioms,
            &BTreeSet::from([ValueId::new(999).expect("unrelated parameter")]),
            &proof,
        ),
        Err(ProofError::IntegerCorrelatedForbiddenRootWitness(
            IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(_)
        ))
    ));

    let forged = ProofNode {
        conclusion: Proposition::Truth,
        rule: proof.rule,
    };
    assert_eq!(
        accept_certificate_with_machine_parameters(
            &context,
            &forged.conclusion,
            &requirements,
            &axioms,
            &BTreeSet::from([ValueId::new(100).expect("root")]),
            &forged,
        ),
        Err(ProofError::IntegerCorrelatedForbiddenRootConversion(
            IntegerCorrelatedForbiddenRootConversionError::ConclusionMismatch,
        ))
    );
}
