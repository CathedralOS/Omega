use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::{EvidenceIdentity, ObligationId, Proposition};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

#[test]
fn exact_add_definition_bound_owns_the_appended_rule_tag() {
    let truth = || ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).expect("nonzero obligation"),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("nonzero evidence identity"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerExactAddDefinitionBound {
                        left_bound: Box::new(truth()),
                        right_bound: Box::new(truth()),
                        definition_axiom: 7,
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("exact-add definition proof encodes");
    assert_eq!(&bytes[8..10], &24_u16.to_le_bytes());
    assert_eq!(bytes[34], 15, "appended exact-add definition rule tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 17;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 17)),
    );
}
