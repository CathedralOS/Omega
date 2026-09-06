use proof_admission::{
    CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker, check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, ObligationId, Proposition, PropositionContext, ScalarTerm, ScalarType,
    ValueId,
};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

fn fixture() -> (PropositionContext, Proposition, ProofBundle) {
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean);
    let premise = Proposition::Equal(value(256), value(1));
    let context = PropositionContext::from_value_types(
        [1, 256].map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean)),
    )
    .unwrap();
    let proof = ProofNode {
        conclusion: Proposition::Equal(value(1), value(256)),
        rule: ProofRule::EqualitySymmetry {
            equality: Box::new(ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).unwrap(),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };
    (context, premise, bundle)
}

#[test]
fn scalar_symmetry_roundtrips_both_directions_without_canonicalizing_proof_citations() {
    let (context, premise, bundle) = fixture();
    let bytes = encode_proof_bundle(&bundle).unwrap();
    assert_eq!(&bytes[8..10], &26_u16.to_le_bytes());
    let decoded = decode_proof_bundle(&bytes).unwrap();
    assert_eq!(decoded, bundle);
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!()
    };
    assert!(matches!(
        certificate.proof.rule,
        ProofRule::EqualitySymmetry { .. }
    ));
    check_certificate(
        &context,
        &certificate.proof.conclusion,
        &[],
        &[premise],
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
}

#[test]
fn symmetry_codec_rejects_truncation_unknown_rule_and_previous_calculus_marker() {
    let (_, _, bundle) = fixture();
    let bytes = encode_proof_bundle(&bundle).unwrap();
    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&24_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(24))
    );
    // Header/envelope use 33 bytes; the Boolean Value equality uses 21.
    assert_eq!(bytes[54], 17);
    let mut unknown = bytes.clone();
    unknown[54] = 19;
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
