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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
    malformed[36] = 24;
    assert_eq!(
        decode_proof_bundle(&malformed),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn transport_counts_are_structural_and_never_preallocate_untrusted_children() {
    // Nonempty, proved equations are a semantic checker requirement. The codec
    // retains the exact child roster, including an invalid empty proof roster.
    let original = bundle(transport(leaf(), Vec::new()));
    let bytes = encode_proof_bundle(&original).unwrap();
    assert_eq!(decode_proof_bundle(&bytes).unwrap(), original);
    assert_eq!(&bytes[38..42], &0_u32.to_le_bytes());
    let mut oversized = bytes.clone();
    oversized[38..42].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_proof_bundle(&oversized).is_err());
    let mut malformed_child =
        encode_proof_bundle(&bundle(transport(leaf(), vec![leaf()]))).unwrap();
    malformed_child[43] = 24;
    assert_eq!(
        decode_proof_bundle(&malformed_child),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn transport_premise_and_equation_children_share_the_proof_depth_limit() {
    let shallow = encode_proof_bundle(&bundle(leaf())).unwrap();
    let leaf_bytes = &shallow[33..36];
    for nested_equation in [false, true] {
        for depth in [256, 257] {
            let mut proof = leaf();
            let mut tree = leaf_bytes.to_vec();
            for _ in 0..depth {
                let mut wrapper = vec![leaf_bytes[0], 23];
                if nested_equation {
                    wrapper.extend_from_slice(leaf_bytes);
                    wrapper.extend_from_slice(&1_u32.to_le_bytes());
                    wrapper.extend_from_slice(&tree);
                    proof = transport(leaf(), vec![proof]);
                } else {
                    wrapper.extend_from_slice(&tree);
                    wrapper.extend_from_slice(&1_u32.to_le_bytes());
                    wrapper.extend_from_slice(leaf_bytes);
                    proof = transport(proof, vec![leaf()]);
                }
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
}
