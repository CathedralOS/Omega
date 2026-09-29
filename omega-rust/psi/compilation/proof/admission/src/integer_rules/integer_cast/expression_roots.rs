//! Cast transport preserves an admitted expression without inventing an SSA root.

use super::{
    IntegerCastChainWitness, IntegerCastChainWitnessError, check_integer_cast_chain_witness,
};
use crate::{MathematicalCoreDecision, ProofNode, ProofRule, accept_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, PropositionError,
    ScalarTerm, ScalarType, ValueId,
};

fn unsigned_type(bits: u16) -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, bits).unwrap()
}

fn value(identity: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type),
    )
}

fn literal(integer_type: IntegerType, number: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap()
}

struct RemainderCast {
    context: PropositionContext,
    source_type: IntegerType,
    target_type: IntegerType,
    root: ScalarTerm,
    target: ScalarTerm,
}

impl RemainderCast {
    fn new() -> Self {
        let source_type = unsigned_type(32);
        let target_type = unsigned_type(8);
        Self {
            context: PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), ScalarType::Integer(source_type)),
                (ValueId::new(2).unwrap(), ScalarType::Integer(target_type)),
            ])
            .unwrap(),
            source_type,
            target_type,
            root: ScalarTerm::exact_integer_remainder(
                source_type,
                value(1, source_type),
                literal(source_type, 256),
            )
            .unwrap(),
            target: value(2, target_type),
        }
    }

    fn definition(&self) -> Proposition {
        // Raw construction also lets malformed-input tests exercise admission.
        Proposition::Equal(
            self.target.clone(),
            ScalarTerm::IntegerExactCast {
                source_type: self.source_type,
                target_type: self.target_type,
                operand: Box::new(self.root.clone()),
            },
        )
    }

    fn witness(&self) -> IntegerCastChainWitness {
        IntegerCastChainWitness {
            root: self.root.clone(),
            target: self.target.clone(),
            definition_axioms: vec![0],
        }
    }
}

#[test]
fn expression_root_transports_only_independently_proved_endpoints() {
    let fixture = RemainderCast::new();
    let definition = fixture.definition();
    for root_is_left in [false, true] {
        let endpoint = if root_is_left { 42 } else { 1 };
        let relation = |root, endpoint| {
            if root_is_left {
                Proposition::LessOrEqual(root, endpoint)
            } else {
                Proposition::LessOrEqual(endpoint, root)
            }
        };
        // This bound is an independent premise, not a fact granted by the cast
        // or by the remainder's carrier. Its exact evidence must survive.
        let root_bound = relation(fixture.root.clone(), literal(fixture.source_type, endpoint));
        let conclusion = relation(
            fixture.target.clone(),
            literal(fixture.target_type, endpoint),
        );
        let proof = ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(ProofNode {
                    conclusion: root_bound.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                witness: fixture.witness(),
            },
        };
        let acceptance = accept_certificate(
            &fixture.context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
            &proof,
        )
        .expect("the admitted expression's exact bound transports through the cast");
        assert!(matches!(
            acceptance.mathematical_core,
            MathematicalCoreDecision::Judged(_)
        ));
        assert_eq!(acceptance.assumptions.len(), 1);
        assert_eq!(acceptance.assumptions[0].proposition, root_bound);
        assert_eq!(acceptance.semantic_axioms.len(), 1);
        assert_eq!(acceptance.semantic_axioms[0].proposition, definition);
        assert!(
            accept_certificate(
                &fixture.context,
                &conclusion,
                &[],
                std::slice::from_ref(&definition),
                &proof,
            )
            .is_err(),
            "the cast equality cannot supply its own missing root-bound premise"
        );

        let stronger_endpoint = if root_is_left {
            endpoint - 1
        } else {
            endpoint + 1
        };
        let stronger = relation(
            fixture.target.clone(),
            literal(fixture.target_type, stronger_endpoint),
        );
        let stronger_proof = ProofNode {
            conclusion: stronger.clone(),
            rule: proof.rule,
        };
        assert!(
            accept_certificate(
                &fixture.context,
                &stronger,
                &[root_bound],
                std::slice::from_ref(&definition),
                &stronger_proof,
            )
            .is_err(),
            "transport cannot strengthen either endpoint"
        );
    }
}

#[test]
fn expression_root_rejects_unknown_and_mistyped_nested_values() {
    let mut fixture = RemainderCast::new();
    fixture.root = ScalarTerm::exact_integer_remainder(
        fixture.source_type,
        value(9, fixture.source_type),
        literal(fixture.source_type, 256),
    )
    .unwrap();
    assert_eq!(
        check_integer_cast_chain_witness(
            &fixture.context,
            &[fixture.definition()],
            &fixture.witness(),
        ),
        Err(IntegerCastChainWitnessError::MalformedProposition(
            PropositionError::UnknownValue(ValueId::new(9).unwrap())
        ))
    );

    fixture.root = ScalarTerm::ExactIntegerRemainder {
        scalar_type: fixture.source_type,
        left: Box::new(value(2, fixture.target_type)),
        right: Box::new(literal(fixture.source_type, 256)),
    };
    assert!(matches!(
        check_integer_cast_chain_witness(
            &fixture.context,
            &[fixture.definition()],
            &fixture.witness(),
        ),
        Err(IntegerCastChainWitnessError::MalformedProposition(_))
    ));

    fixture.root = ScalarTerm::exact_integer_remainder(
        fixture.source_type,
        value(2, fixture.source_type),
        literal(fixture.source_type, 256),
    )
    .unwrap();
    assert!(matches!(
        check_integer_cast_chain_witness(
            &fixture.context,
            &[fixture.definition()],
            &fixture.witness(),
        ),
        Err(IntegerCastChainWitnessError::MalformedProposition(
            PropositionError::ValueTypeMismatch { .. }
        ))
    ));
}

#[test]
fn expression_root_preserves_cast_identity_policy_and_canonical_order() {
    let fixture = RemainderCast::new();
    let definition = fixture.definition();
    let mut witness = fixture.witness();
    witness.root = ScalarTerm::wrapping_integer_remainder(
        fixture.source_type,
        value(1, fixture.source_type),
        literal(fixture.source_type, 256),
    )
    .unwrap();
    assert_eq!(
        check_integer_cast_chain_witness(
            &fixture.context,
            std::slice::from_ref(&definition),
            &witness
        ),
        Err(IntegerCastChainWitnessError::CastChainMismatch(0))
    );

    witness = fixture.witness();
    witness.definition_axioms = vec![0, 0];
    assert_eq!(
        check_integer_cast_chain_witness(
            &fixture.context,
            std::slice::from_ref(&definition),
            &witness
        ),
        Err(IntegerCastChainWitnessError::NonCanonicalDefinitionOrder)
    );

    witness = fixture.witness();
    witness.root = ScalarTerm::Boolean(false);
    assert_eq!(
        check_integer_cast_chain_witness(&fixture.context, &[definition], &witness),
        Err(IntegerCastChainWitnessError::RootNotInteger)
    );
}

#[test]
fn expression_root_rejects_cast_targets_nested_in_the_root() {
    let mut fixture = RemainderCast::new();
    let return_to_source = Proposition::Equal(
        value(1, fixture.source_type),
        ScalarTerm::integer_widen(
            fixture.target_type,
            fixture.source_type,
            fixture.target.clone(),
        )
        .unwrap(),
    );
    let mut witness = fixture.witness();
    witness.definition_axioms.push(1);
    witness.target = value(1, fixture.source_type);
    assert_eq!(
        check_integer_cast_chain_witness(
            &fixture.context,
            &[fixture.definition(), return_to_source],
            &witness,
        ),
        Err(IntegerCastChainWitnessError::CyclicValue(1)),
        "a later cast cannot define a value already used inside the root"
    );

    fixture.root = ScalarTerm::exact_integer_remainder(
        fixture.source_type,
        ScalarTerm::integer_widen(
            fixture.target_type,
            fixture.source_type,
            fixture.target.clone(),
        )
        .unwrap(),
        literal(fixture.source_type, 256),
    )
    .unwrap();
    assert_eq!(
        check_integer_cast_chain_witness(
            &fixture.context,
            &[fixture.definition()],
            &fixture.witness(),
        ),
        Err(IntegerCastChainWitnessError::CyclicValue(0)),
        "the first cast cannot define a value nested inside the root either"
    );
}
