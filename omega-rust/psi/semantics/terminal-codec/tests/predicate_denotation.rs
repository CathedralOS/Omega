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
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
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

#[test]
fn integer_bound_contradiction_reloads_with_both_original_citations() {
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let counter = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(integer));
    let three = ScalarTerm::integer(integer, IntegerValue::Unsigned(3)).unwrap();
    let context = PropositionContext::from_value_types([(
        ValueId::new(1).unwrap(),
        ScalarType::Integer(integer),
    )])
    .unwrap();
    let premises = [
        Proposition::LessOrEqual(three.clone(), counter.clone()),
        Proposition::LessThan(counter, three.clone()),
    ];
    let original = bundle(ProofNode {
        conclusion: Proposition::Falsehood,
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(ProofNode {
                conclusion: Proposition::LessThan(three.clone(), three),
                rule: ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(ProofNode {
                        conclusion: premises[0].clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                    middle_to_right: Box::new(ProofNode {
                        conclusion: premises[1].clone(),
                        rule: ProofRule::Assumption { index: 0 },
                    }),
                },
            }),
        },
    });
    let bytes = encode_proof_bundle(&original).unwrap();
    let decoded = decode_proof_bundle(&bytes).unwrap();
    assert_eq!(decoded, original);
    let EvidenceRoute::CertificateDerived(envelope) = &decoded.evidence[0].route else {
        panic!("certificate route")
    };
    let accepted = proof_admission::accept_certificate(
        &context,
        &Proposition::Falsehood,
        &premises[1..],
        &premises[..1],
        &envelope.proof,
    )
    .unwrap();
    assert_eq!(accepted.assumptions.len(), 1);
    assert_eq!(accepted.assumptions[0].proposition, premises[1]);
    assert_eq!(accepted.semantic_axioms.len(), 1);
    assert_eq!(accepted.semantic_axioms[0].proposition, premises[0]);
    for (assumptions, axioms) in [
        (&[][..], &premises[..1]),
        (&premises[1..], &[][..]),
        (&premises[..1], &premises[1..]),
    ] {
        assert!(
            proof_admission::check_certificate(
                &context,
                &Proposition::Falsehood,
                assumptions,
                axioms,
                &envelope.proof
            )
            .is_err()
        );
    }
    assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
}
