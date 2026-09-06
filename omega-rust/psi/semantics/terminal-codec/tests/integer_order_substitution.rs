use proof_admission::{
    CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker, check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn versioned_order_substitution_keeps_tag_and_exact_strictness() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    for strict in [false, true] {
        let relation = if strict {
            Proposition::LessThan(value(1), value(2))
        } else {
            Proposition::LessOrEqual(value(1), value(2))
        };
        let equality = Proposition::Equal(value(2), value(3));
        let goal = if strict {
            Proposition::LessThan(value(1), value(3))
        } else {
            Proposition::LessOrEqual(value(1), value(3))
        };
        let requirements = [relation.clone(), equality.clone()];
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: ObligationId::new(1).unwrap(),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerOrderSubstitution {
                            relation: Box::new(ProofNode {
                                conclusion: relation,
                                rule: ProofRule::Assumption { index: 0 },
                            }),
                            equality: Box::new(ProofNode {
                                conclusion: equality,
                                rule: ProofRule::Assumption { index: 1 },
                            }),
                            endpoint: 1,
                        },
                    },
                }),
            }],
        };
        let bytes = encode_proof_bundle(&bundle).unwrap();
        assert_eq!(&bytes[8..10], &26_u16.to_le_bytes());
        assert_eq!(
            bytes[60], 11,
            "order substitution retains its versioned payload tag"
        );
        let decoded = decode_proof_bundle(&bytes).unwrap();
        assert_eq!(decoded, bundle);
        assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
        let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
            unreachable!()
        };
        check_certificate(&context, &goal, &requirements, &[], &certificate.proof).unwrap();
        assert!(
            check_certificate(&context, &goal, &requirements[..1], &[], &certificate.proof)
                .is_err()
        );
        let mut changed = certificate.proof.clone();
        changed.conclusion = if strict {
            Proposition::LessOrEqual(value(1), value(3))
        } else {
            Proposition::LessThan(value(1), value(3))
        };
        assert!(
            check_certificate(&context, &changed.conclusion, &requirements, &[], &changed).is_err()
        );
        let mut stale = bytes.clone();
        stale[8..10].copy_from_slice(&25_u16.to_le_bytes());
        assert_eq!(
            decode_proof_bundle(&stale),
            Err(ProofCodecError::UnsupportedFormatMarker(25))
        );
        for length in 0..bytes.len() {
            assert!(
                decode_proof_bundle(&bytes[..length]).is_err(),
                "truncation {length}"
            );
        }
    }
}
