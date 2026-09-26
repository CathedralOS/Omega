//! `IntegerAddOrder` (tag 24) and `IntegerSubtractAntitone` (tag 25): every
//! child round-trips in order, the decoded certificate still checks, and a
//! truncated or rerooted encoding rejects.

use proof_admission::{
    AcceptedProofRule, CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    accept_certificate, check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
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

fn cited(conclusion: &Proposition, index: usize) -> Box<ProofNode> {
    Box::new(ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::Assumption { index },
    })
}

/// The ceiling-distance edge `MAX - (j + 1) < MAX - j`: the advance is an
/// add order and the rank comparison is an antitone subtraction over it.
fn fixture() -> (PropositionContext, Vec<Proposition>, ProofNode) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar = ScalarType::Integer(integer);
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar);
    let literal = |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
    let (ceiling, index, advanced, before, after) =
        (value(1), value(2), value(3), value(4), value(5));
    let premises = vec![
        Proposition::Equal(
            advanced.clone(),
            ScalarTerm::exact_integer_add(integer, index.clone(), literal(1)).unwrap(),
        ),
        Proposition::LessThan(literal(0), literal(1)),
        Proposition::Equal(
            after.clone(),
            ScalarTerm::exact_integer_subtract(integer, ceiling.clone(), advanced.clone()).unwrap(),
        ),
        Proposition::Equal(
            before.clone(),
            ScalarTerm::exact_integer_subtract(integer, ceiling, index.clone()).unwrap(),
        ),
    ];
    let advance = ProofNode {
        conclusion: Proposition::LessThan(index, advanced),
        rule: ProofRule::IntegerAddOrder {
            sum: cited(&premises[0], 0),
            positive: cited(&premises[1], 1),
        },
    };
    let proof = ProofNode {
        conclusion: Proposition::LessThan(after, before),
        rule: ProofRule::IntegerSubtractAntitone {
            smaller: cited(&premises[2], 2),
            larger: cited(&premises[3], 3),
            order: Box::new(advance),
        },
    };
    let context = PropositionContext::from_value_types(
        (1..=5).map(|identity| (ValueId::new(identity).unwrap(), scalar)),
    )
    .unwrap();
    (context, premises, proof)
}

#[test]
fn step_orders_roundtrip_every_child_and_still_check() {
    let (context, premises, proof) = fixture();
    let original = bundle(proof.clone());
    let bytes = encode_proof_bundle(&original).unwrap();
    let decoded = decode_proof_bundle(&bytes).unwrap();
    assert_eq!(decoded, original);
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        panic!("certificate")
    };
    let acceptance = accept_certificate(
        &context,
        &proof.conclusion,
        &premises,
        &[],
        &certificate.proof,
    )
    .unwrap();
    for rule in [
        AcceptedProofRule::IntegerAddOrder,
        AcceptedProofRule::IntegerSubtractAntitone,
    ] {
        assert!(acceptance.rules.contains(&rule), "{rule:?}");
    }
    assert_eq!(acceptance.assumptions.len(), premises.len());
    for length in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..length]).is_err());
    }
}

#[test]
fn step_orders_reject_a_rerooted_difference_or_an_unadvanced_subtrahend() {
    let (context, premises, proof) = fixture();
    for mutation in 0..2 {
        let mut assumptions = premises.clone();
        let mut changed = proof.clone();
        let ProofRule::IntegerSubtractAntitone { larger, order, .. } = &mut changed.rule else {
            panic!("rule")
        };
        match mutation {
            // `before` subtracts from another minuend.
            0 => {
                let Proposition::Equal(before, ScalarTerm::ExactIntegerSubtract { right, .. }) =
                    &premises[3]
                else {
                    panic!("difference")
                };
                assumptions[3] = Proposition::Equal(
                    before.clone(),
                    ScalarTerm::exact_integer_subtract(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        before.clone(),
                        right.as_ref().clone(),
                    )
                    .unwrap(),
                );
                larger.conclusion = assumptions[3].clone();
            }
            // The advance claims the reversed order.
            _ => {
                let Proposition::LessThan(index, advanced) = order.conclusion.clone() else {
                    panic!("order")
                };
                order.conclusion = Proposition::LessThan(advanced, index);
            }
        }
        assert!(
            check_certificate(&context, &changed.conclusion, &assumptions, &[], &changed).is_err(),
            "mutation {mutation}"
        );
        let decoded = decode_proof_bundle(&encode_proof_bundle(&bundle(changed)).unwrap()).unwrap();
        let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
            panic!("certificate")
        };
        assert!(
            check_certificate(
                &context,
                &proof.conclusion,
                &assumptions,
                &[],
                &certificate.proof
            )
            .is_err()
        );
    }
}
