//! Proof-certificate builders shared by exact arithmetic fixtures.

pub(super) fn exact_unsigned_add_certificate(
    integer: semantic_vocabulary::IntegerType,
    left: semantic_vocabulary::ValueId,
    right: semantic_vocabulary::ValueId,
    left_value: semantic_vocabulary::IntegerValue,
    right_value: semantic_vocabulary::IntegerValue,
    left_axiom: usize,
    right_axiom: usize,
    identity: u64,
) -> proof_admission::EvidenceRoute {
    use proof_admission::{
        CertificateEnvelope, IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule,
        ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerMathTerm, Proposition, ScalarTerm, ScalarType,
    };

    let scalar_type = ScalarType::Integer(integer);
    let left_id = left;
    let right_id = right;
    let left = ScalarTerm::value(left, scalar_type);
    let right = ScalarTerm::value(right, scalar_type);
    let left_literal = ScalarTerm::integer(integer, left_value).unwrap();
    let right_literal = ScalarTerm::integer(integer, right_value).unwrap();
    let target = ScalarTerm::exact_integer_add(integer, left.clone(), right.clone()).unwrap();
    let sum = IntegerMathTerm::Add(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value: left_id,
        }),
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value: right_id,
        }),
    );
    let exact_sum = integer.exact_add(left_value, right_value).unwrap();
    let tight = IntegerMathTerm::literal(exact_sum);
    let goal = Proposition::IntegerMathLessOrEqual(
        sum.clone(),
        IntegerMathTerm::literal(integer.maximum_value()),
    );
    let left_equality = Proposition::Equal(left.clone(), left_literal);
    let right_equality = Proposition::Equal(right.clone(), right_literal.clone());
    let right_bound = Proposition::LessOrEqual(right, right_literal.clone());
    let tight_bound = ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(sum, tight.clone()),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    left_equality.clone(),
                    right_bound.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    ProofNode {
                        conclusion: left_equality,
                        rule: ProofRule::SemanticAxiom { index: left_axiom },
                    },
                    ProofNode {
                        conclusion: right_bound,
                        rule: ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: Proposition::LessOrEqual(
                                    right_literal.clone(),
                                    right_literal,
                                ),
                                rule: ProofRule::Primitive(
                                    PrimitiveJudgment::ClosedIntegerRelation,
                                ),
                            }),
                            equality: Box::new(ProofNode {
                                conclusion: right_equality,
                                rule: ProofRule::SemanticAxiom { index: right_axiom },
                            }),
                            endpoint: 0,
                        },
                    },
                ]),
            }),
            witness: IntegerAffineWitness {
                root: left,
                target,
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    };
    proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(identity).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(tight_bound),
                middle_less_or_equal_right: Box::new(ProofNode {
                    conclusion: Proposition::IntegerMathLessOrEqual(
                        tight,
                        IntegerMathTerm::literal(integer.maximum_value()),
                    ),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
            },
        },
    })
}

pub(super) fn exact_unsigned_shift_count_certificate(
    value_type: semantic_vocabulary::IntegerType,
    count_type: semantic_vocabulary::IntegerType,
    count: semantic_vocabulary::ValueId,
    count_axiom: usize,
    identity: u64,
) -> proof_admission::EvidenceRoute {
    use proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType,
    };

    let count = ScalarTerm::value(count, ScalarType::Integer(count_type));
    let zero = ScalarTerm::integer(count_type, IntegerValue::Unsigned(0)).unwrap();
    let maximum = ScalarTerm::integer(
        count_type,
        IntegerValue::Unsigned(u128::from(value_type.bits() - 1)),
    )
    .unwrap();
    let goal = Proposition::LessOrEqual(count.clone(), maximum.clone());
    proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(identity).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(zero.clone(), maximum),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(count, zero),
                    rule: ProofRule::SemanticAxiom { index: count_axiom },
                }),
                endpoint: 0,
            },
        },
    })
}
