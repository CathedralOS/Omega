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
fn order_weakening_roundtrips_and_rejects_missing_or_changed_child_evidence() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let context = PropositionContext::from_value_types(
        [1, 2].map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    for premise in [
        Proposition::Equal(value(1), value(2)),
        Proposition::LessThan(value(1), value(2)),
    ] {
        let bundle = ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence: vec![ObligationEvidence {
                obligation: ObligationId::new(1).unwrap(),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: EvidenceIdentity::new(1).unwrap(),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: Proposition::LessOrEqual(value(1), value(2)),
                        rule: ProofRule::IntegerOrderWeakening {
                            relation: Box::new(ProofNode {
                                conclusion: premise.clone(),
                                rule: ProofRule::Assumption { index: 0 },
                            }),
                        },
                    },
                }),
            }],
        };
        let bytes = encode_proof_bundle(&bundle).unwrap();
        assert_eq!(&bytes[8..10], &25_u16.to_le_bytes());
        // Header/envelope use 33 bytes; the u16 Value relation uses 27.
        assert_eq!(bytes[60], 18);
        let decoded = decode_proof_bundle(&bytes).unwrap();
        assert_eq!(decoded, bundle);
        let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
            unreachable!()
        };
        check_certificate(
            &context,
            &certificate.proof.conclusion,
            &[premise],
            &[],
            &certificate.proof,
        )
        .unwrap();
        assert!(
            check_certificate(
                &context,
                &certificate.proof.conclusion,
                &[],
                &[],
                &certificate.proof
            )
            .is_err()
        );
        assert!(
            check_certificate(
                &context,
                &certificate.proof.conclusion,
                &[Proposition::Truth],
                &[],
                &certificate.proof
            )
            .is_err()
        );
        let mut unknown = bytes.clone();
        unknown[60] = 19;
        assert_eq!(
            decode_proof_bundle(&unknown),
            Err(ProofCodecError::InvalidTag("ProofRule", 19))
        );
        for length in 0..bytes.len() {
            assert!(
                decode_proof_bundle(&bytes[..length]).is_err(),
                "truncation at {length}"
            );
        }
    }
}
