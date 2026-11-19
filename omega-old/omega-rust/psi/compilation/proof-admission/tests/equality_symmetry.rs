use proof_admission::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId};

fn fixture(scalar_type: ScalarType) -> (PropositionContext, Proposition, ProofNode) {
    let context = PropositionContext::from_value_types(
        [1, 256, 257].map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let premise = Proposition::Equal(value(256), value(1));
    let proof = ProofNode {
        conclusion: Proposition::Equal(value(1), value(256)),
        rule: ProofRule::EqualitySymmetry {
            equality: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    (context, premise, proof)
}

#[test]
fn scalar_symmetry_does_not_turn_a_different_relation_into_equality() {
    let (context, _, mut proof) = fixture(ScalarType::Boolean);
    let ProofRule::EqualitySymmetry { equality } = &mut proof.rule else {
        unreachable!()
    };
    equality.conclusion = Proposition::Truth;
    equality.rule = ProofRule::Primitive(proof_admission::PrimitiveJudgment::Truth);
    assert!(check_certificate(&context, &proof.conclusion, &[], &[], &proof).is_err());
}
