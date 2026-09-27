use proof_admission::{
    AcceptedProofRule, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    accept_certificate, check_certificate, lift_fixed_integer_relation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, PropositionContext,
    ScalarTerm, ScalarType, ValueId,
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
            assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
            assert_eq!(&bytes[31..33], &5_u16.to_le_bytes());
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
