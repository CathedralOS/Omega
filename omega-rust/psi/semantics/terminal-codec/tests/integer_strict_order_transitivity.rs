use proof_admission::{
    AcceptedProofRule, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, accept_certificate, check_certificate, lift_fixed_integer_relation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
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

fn fixture(
    strict: [bool; 2],
    mathematical: bool,
) -> (PropositionContext, [Proposition; 2], ProofNode) {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
    let context = PropositionContext::from_value_types(
        (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
    )
    .unwrap();
    let order = |left, right, strict| {
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let proposition = if strict {
            Proposition::LessThan(value(left), value(right))
        } else {
            Proposition::LessOrEqual(value(left), value(right))
        };
        if mathematical {
            lift_fixed_integer_relation(&proposition).unwrap()
        } else {
            proposition
        }
    };
    let premises = [order(1, 2, strict[0]), order(2, 3, strict[1])];
    let proof = ProofNode {
        conclusion: order(1, 3, true),
        rule: ProofRule::IntegerStrictOrderTransitivity {
            left_to_middle: Box::new(ProofNode {
                conclusion: premises[0].clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            middle_to_right: Box::new(ProofNode {
                conclusion: premises[1].clone(),
                rule: ProofRule::SemanticAxiom { index: 0 },
            }),
        },
    };
    (context, premises, proof)
}

#[test]
fn strict_and_mixed_chains_roundtrip_and_replay_both_citations() {
    for strict in [[true, false], [false, true], [true, true]] {
        for mathematical in [false, true] {
            let (context, premises, proof) = fixture(strict, mathematical);
            let original = bundle(proof.clone());
            let bytes = encode_proof_bundle(&original).unwrap();
            assert_eq!(&bytes[8..10], &31_u16.to_le_bytes());
            assert_eq!(&bytes[31..33], &3_u16.to_le_bytes());
            let decoded = decode_proof_bundle(&bytes).unwrap();
            assert_eq!(decoded, original);
            assert_eq!(encode_proof_bundle(&decoded).unwrap(), bytes);
            let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
                unreachable!()
            };
            let accepted = accept_certificate(
                &context,
                &proof.conclusion,
                &premises[..1],
                &premises[1..],
                &certificate.proof,
            )
            .unwrap();
            assert!(
                accepted
                    .rules
                    .contains(&AcceptedProofRule::IntegerStrictOrderTransitivity)
            );
            assert_eq!(accepted.assumptions.len(), 1);
            assert_eq!(accepted.semantic_axioms.len(), 1);
            for (requirements, axioms) in [(&[][..], &premises[1..]), (&premises[..1], &[][..])] {
                assert!(
                    check_certificate(
                        &context,
                        &proof.conclusion,
                        requirements,
                        axioms,
                        &certificate.proof
                    )
                    .is_err()
                );
            }
            let mut swapped = certificate.proof.clone();
            let ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } = &mut swapped.rule
            else {
                unreachable!()
            };
            std::mem::swap(left_to_middle, middle_to_right);
            assert!(
                check_certificate(
                    &context,
                    &proof.conclusion,
                    &premises[..1],
                    &premises[1..],
                    &swapped
                )
                .is_err()
            );
        }
    }
}

#[test]
fn appended_rule_tag_and_current_markers_reject_unknown_or_stale_bytes() {
    let (_, _, proof) = fixture([true, false], false);
    let bytes = encode_proof_bundle(&bundle(proof.clone())).unwrap();
    let ProofRule::IntegerStrictOrderTransitivity {
        left_to_middle,
        middle_to_right,
    } = proof.rule
    else {
        unreachable!()
    };
    let previous_rule = ProofNode {
        conclusion: proof.conclusion,
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: left_to_middle,
            middle_less_or_equal_right: middle_to_right,
        },
    };
    let previous_bytes = encode_proof_bundle(&bundle(previous_rule)).unwrap();
    let differences = bytes
        .iter()
        .zip(&previous_bytes)
        .enumerate()
        .filter_map(|(position, (current, previous))| (current != previous).then_some(position))
        .collect::<Vec<_>>();
    assert_eq!(differences.len(), 1, "only the rule tag differs");
    let rule_position = differences[0];
    assert_eq!(bytes[rule_position], 21);
    assert_eq!(previous_bytes[rule_position], 10);
    let mut unknown = bytes.clone();
    unknown[rule_position] = 22;
    assert_eq!(
        decode_proof_bundle(&unknown),
        Err(ProofCodecError::InvalidTag("ProofRule", 22))
    );
    let mut stale_format = bytes.clone();
    stale_format[8..10].copy_from_slice(&30_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale_format),
        Err(ProofCodecError::UnsupportedFormatMarker(30))
    );
    let mut stale_system = bytes.clone();
    stale_system[31..33].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale_system),
        Err(ProofCodecError::UnsupportedProofSystemMarker(2))
    );
    for length in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..length]).is_err());
    }
}

#[test]
fn both_order_children_obey_encoding_and_decoding_depth_limits() {
    let leaf = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let shallow = encode_proof_bundle(&bundle(leaf.clone())).unwrap();
    let leaf_bytes = &shallow[33..36];
    for nested_right in [false, true] {
        for depth in [256, 257] {
            let mut tree = leaf_bytes.to_vec();
            let mut proof = leaf.clone();
            for _ in 0..depth {
                let mut wrapper = vec![leaf_bytes[0], 21];
                let other = Box::new(leaf.clone());
                let nested = Box::new(proof);
                let (left_to_middle, middle_to_right) = if nested_right {
                    wrapper.extend_from_slice(leaf_bytes);
                    wrapper.extend_from_slice(&tree);
                    (other, nested)
                } else {
                    wrapper.extend_from_slice(&tree);
                    wrapper.extend_from_slice(leaf_bytes);
                    (nested, other)
                };
                tree = wrapper;
                proof = ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle,
                        middle_to_right,
                    },
                };
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
