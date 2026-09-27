use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, check_certificate};
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
