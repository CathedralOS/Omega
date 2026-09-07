use super::{kernel_bundle, semantic_module};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    CycleComponentId, EvidenceIdentity, ObligationId, Proposition, RankingRelationId,
    RecursiveComponentId,
};
use terminal_codec::{
    ProofCodecError, decode_proof_bundle, encode_proof_bundle, proof_bundle_fingerprint,
};
use terminal_psi::{ControlCycleEvidence, ProofBundle, RecursiveComponentEvidence};
use terminal_verifier::verify_module;

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
fn control_cycle_certificates_round_trip_after_proof_recursion_with_exact_identities() {
    let control = component(31);
    let bundle = ProofBundle {
        recursive_components: vec![RecursiveComponentEvidence {
            component: RecursiveComponentId::new(31).unwrap(),
            certificate: control.certificate.clone(),
        }],
        control_cycles: vec![control, component(32)],
        ..ProofBundle::default()
    };
    let bytes = encode_proof_bundle(&bundle).unwrap();
    assert_eq!(&bytes[8..10], &30_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    let identity = proof_bundle_fingerprint(&bundle).unwrap();
    for changed in [
        {
            let mut changed = bundle.clone();
            changed.control_cycles[0].component = CycleComponentId::new(30).unwrap();
            changed
        },
        {
            let mut changed = bundle.clone();
            changed.control_cycles[0].certificate.identity = EvidenceIdentity::new(42).unwrap();
            changed
        },
        {
            let mut changed = bundle.clone();
            changed.control_cycles[0].certificate.ranking_relation =
                RankingRelationId::new(52).unwrap();
            changed
        },
        {
            let mut changed = bundle.clone();
            changed.control_cycles[0].certificate.edges[0].obligation =
                ObligationId::new(60).unwrap();
            changed
        },
    ] {
        assert_ne!(proof_bundle_fingerprint(&changed).unwrap(), identity);
        assert_eq!(
            decode_proof_bundle(&encode_proof_bundle(&changed).unwrap()),
            Ok(changed)
        );
    }
    let mut stale = bytes;
    stale[8..10].copy_from_slice(&29_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(29)),
    );
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
    // through the two component payloads (excluding the final producer count).
    let component_size = (bytes.len() - 22 - 4) / 2;
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
fn wire_control_cycle_evidence_never_authorizes_surplus_components() {
    let module = semantic_module();
    let mut bundle = kernel_bundle();
    bundle.control_cycles.push(component(31));
    let decoded = decode_proof_bundle(&encode_proof_bundle(&bundle).unwrap()).unwrap();
    assert!(verify_module(&module, &decoded, &AdmissionProfile::default()).is_err());
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
