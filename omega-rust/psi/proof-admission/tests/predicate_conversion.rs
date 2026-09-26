use proof_admission::{
    AcceptedProofRule, PrimitiveJudgment, ProofNode, ProofRule, accept_certificate,
    check_certificate,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};

fn context() -> PropositionContext {
    PropositionContext::from_value_types(
        (1..=2).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean)
}

fn converted(conclusion: Proposition, premise: ProofNode) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(premise),
        },
    }
}

#[test]
fn conversion_keeps_original_citation_and_exact_outer_conclusion() {
    let original = Proposition::Equal(
        ScalarTerm::boolean_not(value(1)).unwrap(),
        ScalarTerm::boolean(true),
    );
    let goal = Proposition::Equal(value(1), ScalarTerm::boolean(false));
    for semantic in [false, true] {
        let proof = converted(
            goal.clone(),
            ProofNode {
                conclusion: original.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index: 0 }
                } else {
                    ProofRule::Assumption { index: 0 }
                },
            },
        );
        let (assumptions, axioms) = if semantic {
            (vec![], vec![original.clone()])
        } else {
            (vec![original.clone()], vec![])
        };
        let acceptance =
            accept_certificate(&context(), &goal, &assumptions, &axioms, &proof).unwrap();
        assert!(
            acceptance
                .rules
                .contains(&AcceptedProofRule::PredicateDenotation)
        );
        let citations = if semantic {
            &acceptance.semantic_axioms
        } else {
            &acceptance.assumptions
        };
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].index, 0);
        assert_eq!(citations[0].proposition, original);
        assert!(check_certificate(&context(), &goal, &[], &[], &proof).is_err());
        assert!(check_certificate(&context(), &original, &assumptions, &axioms, &proof).is_err());
        let normalized_only = vec![goal.clone()];
        assert!(
            check_certificate(
                &context(),
                &goal,
                if semantic { &[] } else { &normalized_only },
                if semantic { &normalized_only } else { &[] },
                &proof
            )
            .is_err()
        );
    }
}

#[test]
fn conversion_rejects_wrong_polarity_identity_type_and_unproved_child() {
    let original = Proposition::Equal(value(1), ScalarTerm::boolean(false));
    for goal in [
        Proposition::Equal(value(1), ScalarTerm::boolean(true)),
        Proposition::Equal(value(2), ScalarTerm::boolean(false)),
        Proposition::Equal(
            value(1),
            ScalarTerm::integer(
                semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Unsigned,
                    8,
                )
                .unwrap(),
                semantic_vocabulary::IntegerValue::Unsigned(0),
            )
            .unwrap(),
        ),
    ] {
        let proof = converted(
            goal.clone(),
            ProofNode {
                conclusion: original.clone(),
                rule: ProofRule::Assumption { index: 0 },
            },
        );
        assert!(
            check_certificate(
                &context(),
                &goal,
                std::slice::from_ref(&original),
                &[],
                &proof
            )
            .is_err()
        );
    }
    let proof = converted(
        original.clone(),
        ProofNode {
            conclusion: original.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        },
    );
    assert!(check_certificate(&context(), &original, &[], &[], &proof).is_err());
}

#[test]
fn conversion_discharges_only_its_local_implication_assumption() {
    let original = Proposition::Equal(value(1), ScalarTerm::boolean(false));
    let goal = Proposition::Equal(
        ScalarTerm::boolean_not(value(1)).unwrap(),
        ScalarTerm::boolean(true),
    );
    let proof = ProofNode {
        conclusion: Proposition::Implication {
            premise: Box::new(original.clone()),
            conclusion: Box::new(goal.clone()),
        },
        rule: ProofRule::ImplicationIntroduction {
            body: Box::new(converted(
                goal,
                ProofNode {
                    conclusion: original,
                    rule: ProofRule::Assumption { index: 0 },
                },
            )),
        },
    };
    let acceptance = accept_certificate(&context(), &proof.conclusion, &[], &[], &proof).unwrap();
    assert!(acceptance.assumptions.is_empty());
    assert!(acceptance.semantic_axioms.is_empty());
}

#[test]
fn conversion_cannot_bypass_denotation_resource_limits() {
    let mut term = value(1);
    for _ in 0..65 {
        term = ScalarTerm::BooleanNot {
            operand: Box::new(term),
        };
    }
    let deep = Proposition::Equal(term.clone(), term);
    let proof = converted(
        deep.clone(),
        ProofNode {
            conclusion: deep.clone(),
            rule: ProofRule::Assumption { index: 0 },
        },
    );
    assert!(
        check_certificate(&context(), &deep, std::slice::from_ref(&deep), &[], &proof).is_err()
    );
}
