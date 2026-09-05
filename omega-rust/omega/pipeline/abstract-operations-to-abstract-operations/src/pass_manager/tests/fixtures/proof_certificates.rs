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
                        rule: ProofRule::IntegerLessOrEqualSubstitution {
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
            rule: ProofRule::IntegerLessOrEqualSubstitution {
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

pub(super) fn remainder_by_one_certificate(
    integer: semantic_vocabulary::IntegerType,
    divisor: semantic_vocabulary::ValueId,
) -> proof_admission::EvidenceRoute {
    use proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType,
    };

    let scalar_type = ScalarType::Integer(integer);
    let literal_one = ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let divisor_term = ScalarTerm::value(divisor, scalar_type);
    proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(462).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: Proposition::LessOrEqual(literal_one.clone(), divisor_term.clone()),
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(literal_one.clone(), literal_one.clone()),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Equal(divisor_term, literal_one),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                endpoint: 1,
            },
        },
    })
}

pub(super) fn signed_remainder_by_negative_one_certificate(
    integer: semantic_vocabulary::IntegerType,
    dividend: semantic_vocabulary::ValueId,
    divisor: semantic_vocabulary::ValueId,
) -> proof_admission::EvidenceRoute {
    use proof_admission::{
        CertificateEnvelope, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerValue, Proposition, ScalarTerm, ScalarType,
    };

    let scalar_type = ScalarType::Integer(integer);
    let literal = |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).unwrap();
    let dividend_term = ScalarTerm::value(dividend, scalar_type);
    let divisor_term = ScalarTerm::value(divisor, scalar_type);
    let minimum_plus_one = match integer.minimum_value() {
        IntegerValue::Signed(minimum) => minimum.checked_add(1).unwrap(),
        IntegerValue::Unsigned(_) => unreachable!("negative-one fixture requires a signed type"),
    };
    let negative_case = Proposition::LessOrEqual(divisor_term.clone(), literal(-1));
    let dividend_case = Proposition::LessOrEqual(literal(minimum_plus_one), dividend_term.clone());
    let defined_case = Proposition::Conjunction(vec![negative_case.clone(), dividend_case.clone()]);
    let goal = Proposition::Disjunction(vec![
        Proposition::LessOrEqual(divisor_term.clone(), literal(-2)),
        Proposition::LessOrEqual(literal(1), divisor_term.clone()),
        defined_case.clone(),
    ]);
    let prove_bound = |conclusion: Proposition,
                       relation: Proposition,
                       equality: Proposition,
                       endpoint: usize,
                       axiom: usize| ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(ProofNode {
                conclusion: relation,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            }),
            equality: Box::new(ProofNode {
                conclusion: equality,
                rule: ProofRule::SemanticAxiom { index: axiom },
            }),
            endpoint,
        },
    };
    proof_admission::EvidenceRoute::CertificateDerived(CertificateEnvelope {
        identity: EvidenceIdentity::new(483).unwrap(),
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof: ProofNode {
            conclusion: goal,
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(ProofNode {
                    conclusion: defined_case,
                    rule: ProofRule::ConjunctionIntroduction(vec![
                        prove_bound(
                            negative_case,
                            Proposition::LessOrEqual(literal(-1), literal(-1)),
                            Proposition::Equal(divisor_term, literal(-1)),
                            0,
                            1,
                        ),
                        prove_bound(
                            dividend_case,
                            Proposition::LessOrEqual(literal(minimum_plus_one), literal(7)),
                            Proposition::Equal(dividend_term, literal(7)),
                            1,
                            0,
                        ),
                    ]),
                }),
                index: 2,
            },
        },
    })
}
