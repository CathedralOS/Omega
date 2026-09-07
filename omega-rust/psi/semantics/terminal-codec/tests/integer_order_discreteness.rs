use proof_admission::{
    AcceptedProofRule, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    accept_certificate, check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
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

#[test]
fn order_discreteness_roundtrips_exact_child_and_replays_acceptance_trace() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let value = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
    let literal = |value| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(value)).unwrap();
    let context =
        PropositionContext::from_value_types([(ValueId::new(1).unwrap(), scalar_type)]).unwrap();
    for (premise, conclusion) in [
        (
            Proposition::LessOrEqual(literal(1), value.clone()),
            Proposition::LessThan(literal(0), value.clone()),
        ),
        (
            Proposition::LessOrEqual(value.clone(), literal(254)),
            Proposition::LessThan(value.clone(), literal(255)),
        ),
    ] {
        let original = bundle(ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(ProofNode {
                    conclusion: premise.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        });
        let bytes = encode_proof_bundle(&original).unwrap();
        assert_eq!(&bytes[8..10], &28_u16.to_le_bytes());
        let positions = bytes
            .iter()
            .enumerate()
            .filter_map(|(position, byte)| (*byte == 19).then_some(position))
            .collect::<Vec<_>>();
        let [tag] = positions.as_slice() else {
            panic!("one order-discreteness tag in this fixture")
        };
        let decoded = decode_proof_bundle(&bytes).unwrap();
        assert_eq!(decoded, original);
        let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
            unreachable!()
        };
        let acceptance = accept_certificate(
            &context,
            &conclusion,
            std::slice::from_ref(&premise),
            &[],
            &certificate.proof,
        )
        .unwrap();
        assert_eq!(
            acceptance.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerOrderDiscreteness
            ]
        );
        assert_eq!(acceptance.assumptions.len(), 1);
        assert_eq!(acceptance.assumptions[0].proposition, premise);
        for assumptions in [Vec::new(), vec![Proposition::Truth]] {
            assert!(
                check_certificate(&context, &conclusion, &assumptions, &[], &certificate.proof)
                    .is_err()
            );
        }
        let mut unknown = bytes.clone();
        unknown[*tag] = 20;
        assert_eq!(
            decode_proof_bundle(&unknown),
            Err(ProofCodecError::InvalidTag("ProofRule", 20))
        );
        let mut changed_rule = bytes.clone();
        changed_rule[*tag] = 18;
        let changed = decode_proof_bundle(&changed_rule).unwrap();
        let EvidenceRoute::CertificateDerived(changed_certificate) = &changed.evidence[0].route
        else {
            unreachable!()
        };
        assert!(
            check_certificate(
                &context,
                &conclusion,
                &[premise],
                &[],
                &changed_certificate.proof
            )
            .is_err()
        );
        for length in 0..bytes.len() {
            assert!(
                decode_proof_bundle(&bytes[..length]).is_err(),
                "truncated at {length}"
            );
        }
    }
}

#[test]
fn order_discreteness_child_counts_toward_proof_nesting_limit() {
    let mut proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Assumption { index: 0 },
    };
    for _ in 0..257 {
        proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::IntegerOrderDiscreteness {
                relation: Box::new(proof),
            },
        };
    }
    assert_eq!(
        encode_proof_bundle(&bundle(proof)),
        Err(ProofCodecError::ProofNestingTooDeep)
    );
}
