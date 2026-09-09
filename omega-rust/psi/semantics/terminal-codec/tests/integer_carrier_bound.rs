use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerSign, IntegerType, IntegerValue, ObligationId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use terminal_codec::{decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn integer_carrier_bound_certificate_roundtrips_without_assumptions() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = ValueId::new(1).unwrap();
    let goal = Proposition::LessOrEqual(
        ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap(),
        ScalarTerm::value(value, ScalarType::Integer(integer)),
    );
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
    check_certificate(&context, &goal, &[], &[], &proof).unwrap();
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).unwrap(),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
        ..ProofBundle::default()
    };
    let bytes = encode_proof_bundle(&bundle).unwrap();
    assert_eq!(&bytes[8..10], &31_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes).unwrap(), bundle);
    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&27_u16.to_le_bytes());
    assert!(decode_proof_bundle(&stale).is_err());
    for length in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..length]).is_err());
    }
}
