use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
    RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    CycleComponentId, EvidenceIdentity, ObligationId, Proposition, RankingRelationId,
};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
use terminal_psi::{ControlCycleEvidence, ProofBundle};

fn component(identity: u64) -> ControlCycleEvidence {
    ControlCycleEvidence {
        component: CycleComponentId::new(identity).unwrap(),
        certificate: RecursiveComponentCertificate {
            identity: EvidenceIdentity::new(41).unwrap(),
            ranking_relation: RankingRelationId::new(51).unwrap(),
            well_foundedness: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            edges: vec![
                RecursiveEdgeCertificate {
                    obligation: ObligationId::new(61).unwrap(),
                    evidence: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                        identity: EvidenceIdentity::new(71).unwrap(),
                        proof_system_marker: ProofSystemMarker::CURRENT,
                        proof: ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                        },
                    }),
                },
                RecursiveEdgeCertificate {
                    obligation: ObligationId::new(62).unwrap(),
                    evidence: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
                },
            ],
        },
    }
}

#[test]
fn control_cycle_evidence_rejects_noncanonical_components_and_edges() {
    let bundle = ProofBundle {
        control_cycles: vec![component(31), component(32)],
        ..ProofBundle::default()
    };
    let mut reordered = bundle.clone();
    reordered.control_cycles.reverse();
    let mut duplicate = bundle.clone();
    duplicate.control_cycles[1].component = duplicate.control_cycles[0].component;
    let mut reordered_edges = bundle.clone();
    reordered_edges.control_cycles[0]
        .certificate
        .edges
        .reverse();
    let mut duplicate_edge = bundle.clone();
    duplicate_edge.control_cycles[0].certificate.edges[1].obligation =
        ObligationId::new(61).unwrap();
    for changed in [reordered, duplicate, reordered_edges, duplicate_edge] {
        assert_eq!(
            encode_proof_bundle(&changed),
            Err(ProofCodecError::NonCanonicalControlCycleEvidence),
        );
    }
    let bytes = encode_proof_bundle(&bundle).unwrap();
    // Empty flat evidence and proof-recursion rosters precede control cycles.
    assert_eq!(&bytes[10..22], &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]);
    let mut zero_identity = bytes.clone();
    zero_identity[22..30].fill(0);
    assert_eq!(
        decode_proof_bundle(&zero_identity),
        Err(ProofCodecError::ZeroIdentity("CycleComponentId")),
    );
    // Equal-size certificates make the second component start exactly halfway
    // through the two component payloads (excluding the final crash-roster
    // and producer counts).
    let component_size = (bytes.len() - 22 - 8) / 2;
    let mut duplicate_wire = bytes.clone();
    duplicate_wire[22 + component_size..30 + component_size].copy_from_slice(&31_u64.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&duplicate_wire),
        Err(ProofCodecError::NonCanonicalControlCycleEvidence),
    );
    for end in 0..bytes.len() {
        assert!(decode_proof_bundle(&bytes[..end]).is_err());
    }
}

#[test]
fn control_cycle_routes_enforce_the_same_proof_depth_limit() {
    let mut bundle = ProofBundle {
        control_cycles: vec![component(31)],
        ..ProofBundle::default()
    };
    let mut proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    for _ in 0..258 {
        proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::ConjunctionIntroduction(vec![proof]),
        };
    }
    bundle.control_cycles[0].certificate.well_foundedness =
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(72).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof,
        });
    assert_eq!(
        encode_proof_bundle(&bundle),
        Err(ProofCodecError::ProofNestingTooDeep),
    );
}
