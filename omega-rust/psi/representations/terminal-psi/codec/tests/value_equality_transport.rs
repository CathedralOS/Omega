use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{
    EvidenceIdentity, ObligationId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
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

fn leaf() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn transport(premise: ProofNode, equalities: Vec<ProofNode>) -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::ValueEqualityTransport {
            premise: Box::new(premise),
            equalities,
        },
    }
}

#[test]
fn transport_roundtrip_preserves_nested_premise_and_ordered_equations() {
    let equation = |identity, index| ProofNode {
        conclusion: Proposition::Equal(
            ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean),
            ScalarTerm::boolean(false),
        ),
        rule: ProofRule::SemanticAxiom { index },
    };
    let original = bundle(transport(
        ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(leaf()),
            },
        },
        vec![equation(2, 7), equation(1, 3)],
    ));
    let bytes = encode_proof_bundle(&original).unwrap();
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(&bytes[31..33], &5_u16.to_le_bytes());
    assert_eq!(bytes[34], 23);
    let decoded = decode_proof_bundle(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
    for length in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..length]).is_err());
    }
    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&32_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(32))
    );
    let mut stale = bytes.clone();
    stale[31..33].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedProofSystemMarker(4))
    );
    let mut malformed = bytes.clone();
    malformed[36] = 27;
    assert_eq!(
        decode_proof_bundle(&malformed),
        Err(ProofCodecError::InvalidTag("ProofRule", 27))
    );
}
