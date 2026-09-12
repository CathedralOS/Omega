use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{EvidenceIdentity, ObligationId, Proposition};
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

fn conversion(premise: ProofNode) -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(premise),
        },
    }
}

#[test]
fn nested_predicate_denotation_preserves_exact_child_and_current_markers() {
    let original = bundle(conversion(conversion(ProofNode {
        conclusion: Proposition::Falsehood,
        rule: ProofRule::Assumption { index: 7 },
    })));
    // The codec retains evidence structure; semantic equivalence and premise
    // availability are checked by the proof owner, not invented by decoding.
    let bytes = encode_proof_bundle(&original).unwrap();
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
    assert_eq!(&bytes[31..33], &5_u16.to_le_bytes());
    assert_eq!(bytes[34], 22);
    let decoded = decode_proof_bundle(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&31_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(31))
    );
    let mut stale = bytes.clone();
    stale[31..33].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedProofSystemMarker(3))
    );
    let mut unknown = bytes.clone();
    unknown[36] = 24;
    assert_eq!(
        decode_proof_bundle(&unknown),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
    for length in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..length]).is_err());
    }
}

#[test]
fn predicate_denotation_child_obeys_encoding_and_decoding_depth_limits() {
    let shallow = encode_proof_bundle(&bundle(leaf())).unwrap();
    let leaf_bytes = &shallow[33..36];
    for depth in [256, 257] {
        let mut proof = leaf();
        let mut tree = leaf_bytes.to_vec();
        for _ in 0..depth {
            proof = conversion(proof);
            let mut wrapper = vec![leaf_bytes[0], 22];
            wrapper.extend_from_slice(&tree);
            tree = wrapper;
        }
        let mut bytes = shallow[..33].to_vec();
        bytes.extend_from_slice(&tree);
        bytes.extend_from_slice(&shallow[36..]);
        if depth == 256 {
            assert_eq!(encode_proof_bundle(&bundle(proof)).unwrap(), bytes);
            let decoded = decode_proof_bundle(&bytes).unwrap();
            assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
        } else {
            assert_eq!(
                encode_proof_bundle(&bundle(proof)),
                Err(ProofCodecError::ProofNestingTooDeep)
            );
            assert_eq!(
                decode_proof_bundle(&bytes),
                Err(ProofCodecError::ProofNestingTooDeep)
            );
        }
    }
}
