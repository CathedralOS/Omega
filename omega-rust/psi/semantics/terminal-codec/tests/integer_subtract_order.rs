use proof_admission::{
    AcceptedProofRule, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    accept_certificate, check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

fn bundle(proof: ProofNode) -> ProofBundle {
    ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).unwrap(),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
        ..ProofBundle::default()
    }
}

fn fixture(sign: IntegerSign, width: u16) -> (PropositionContext, Vec<Proposition>, ProofNode) {
    let integer_type = IntegerType::new(sign, width).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let zero = ScalarTerm::integer(
        integer_type,
        match sign {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        },
    )
    .unwrap();
    let context = PropositionContext::from_value_types(
        (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let premises = vec![
        Proposition::Equal(
            value(3),
            ScalarTerm::exact_integer_subtract(integer_type, value(1), value(2)).unwrap(),
        ),
        Proposition::LessThan(zero, value(2)),
    ];
    let proof = ProofNode {
        conclusion: Proposition::LessThan(value(3), value(1)),
        rule: ProofRule::IntegerSubtractOrder {
            difference: Box::new(ProofNode {
                conclusion: premises[0].clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            positive: Box::new(ProofNode {
                conclusion: premises[1].clone(),
                rule: ProofRule::Assumption { index: 1 },
            }),
        },
    };
    (context, premises, proof)
}

#[test]
fn subtraction_order_roundtrips_both_citations_and_rejects_stale_format() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        for width in [8, 64, 128] {
            let (context, premises, proof) = fixture(sign, width);
            let original = bundle(proof.clone());
            let bytes = encode_proof_bundle(&original).unwrap();
            assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
            let decoded = decode_proof_bundle(&bytes).unwrap();
            assert_eq!(decoded, original);
            let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
                panic!("certificate")
            };
            let acceptance = accept_certificate(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &certificate.proof,
            )
            .unwrap();
            assert!(
                acceptance
                    .rules
                    .contains(&AcceptedProofRule::IntegerSubtractOrder)
            );
            assert_eq!(acceptance.assumptions.len(), 2);
            for omitted in 0..2 {
                let mut changed = premises.clone();
                changed[omitted] = Proposition::Truth;
                assert!(
                    check_certificate(
                        &context,
                        &proof.conclusion,
                        &changed,
                        &[],
                        &certificate.proof
                    )
                    .is_err()
                );
            }
            let mut stale = bytes.clone();
            stale[8..10].copy_from_slice(&28_u16.to_le_bytes());
            assert!(decode_proof_bundle(&stale).is_err());
            for length in 0..bytes.len() {
                assert!(decode_proof_bundle(&bytes[..length]).is_err());
            }
        }
    }
}

#[test]
fn subtraction_order_rejects_false_premise_shapes_and_redirected_results() {
    let (context, premises, proof) = fixture(IntegerSign::Unsigned, 64);
    for mutation in 0..7 {
        let mut changed = proof.clone();
        let mut assumptions = premises.clone();
        let ProofRule::IntegerSubtractOrder {
            difference,
            positive,
        } = &mut changed.rule
        else {
            panic!("rule")
        };
        let Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) = &premises[0]
        else {
            panic!("equation")
        };
        let Proposition::LessThan(zero, decrement) = &premises[1] else {
            panic!("positivity")
        };
        match mutation {
            0 => positive.conclusion = Proposition::LessOrEqual(zero.clone(), decrement.clone()),
            1 => positive.conclusion = Proposition::LessThan(decrement.clone(), zero.clone()),
            2 => positive.conclusion = Proposition::LessThan(zero.clone(), left.as_ref().clone()),
            3 => {
                difference.conclusion = Proposition::Equal(
                    result.clone(),
                    ScalarTerm::wrapping_integer_subtract(
                        *scalar_type,
                        left.as_ref().clone(),
                        right.as_ref().clone(),
                    )
                    .unwrap(),
                )
            }
            4 => {
                difference.conclusion = Proposition::Equal(
                    result.clone(),
                    ScalarTerm::exact_integer_subtract(
                        *scalar_type,
                        right.as_ref().clone(),
                        left.as_ref().clone(),
                    )
                    .unwrap(),
                )
            }
            5 => changed.conclusion = Proposition::LessThan(left.as_ref().clone(), result.clone()),
            6 => changed.conclusion = Proposition::LessThan(result.clone(), right.as_ref().clone()),
            _ => unreachable!(),
        }
        assumptions[0] = difference.conclusion.clone();
        assumptions[1] = positive.conclusion.clone();
        assert!(
            check_certificate(&context, &changed.conclusion, &assumptions, &[], &changed).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn subtraction_order_both_children_count_toward_nesting_limit() {
    let (_, _, proof) = fixture(IntegerSign::Unsigned, 8);
    for nested_positive in [false, true] {
        let mut nested = proof.clone();
        for _ in 0..257 {
            let child = Box::new(nested);
            let other = Box::new(proof.clone());
            let (difference, positive) = if nested_positive {
                (other, child)
            } else {
                (child, other)
            };
            nested = ProofNode {
                conclusion: proof.conclusion.clone(),
                rule: ProofRule::IntegerSubtractOrder {
                    difference,
                    positive,
                },
            };
        }
        assert!(encode_proof_bundle(&bundle(nested)).is_err());
    }
}
