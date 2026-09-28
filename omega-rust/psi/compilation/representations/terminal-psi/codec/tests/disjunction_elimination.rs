use proof_admission::{
    CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofError, ProofNode, ProofRule,
    ProofSystemMarker, accept_certificate, check_certificate,
};
use semantic_vocabulary::{EvidenceIdentity, ObligationId, Proposition, PropositionContext};
use terminal_codec::{ProofCodecError, decode_proof_bundle, encode_proof_bundle};
use terminal_verifier::{ObligationEvidence, ProofBundle};

fn truth() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    }
}

fn proof() -> ProofNode {
    ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::DisjunctionElimination {
            disjunction: Box::new(ProofNode {
                conclusion: Proposition::Disjunction(vec![
                    Proposition::Truth,
                    Proposition::Falsehood,
                ]),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(truth()),
                    index: 0,
                },
            }),
            branches: vec![
                ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Assumption { index: 0 },
                },
                truth(),
            ],
        },
    }
}

fn bundle(proof: ProofNode) -> ProofBundle {
    ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: ObligationId::new(1).expect("obligation"),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).expect("evidence"),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    }
}

fn decoded_proof(bytes: &[u8]) -> ProofNode {
    let decoded = decode_proof_bundle(bytes).expect("canonical proof bytes");
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!()
    };
    certificate.proof.clone()
}

#[test]
fn case_analysis_roundtrips_with_explicit_ordered_branches() {
    let bundle = bundle(proof());
    let bytes = encode_proof_bundle(&bundle).expect("case analysis encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 16, "appended disjunction elimination rule tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
    let proof = decoded_proof(&bytes);
    let accepted = accept_certificate(
        &PropositionContext::default(),
        &Proposition::Truth,
        &[],
        &[],
        &proof,
    )
    .expect("independently checked case analysis");
    assert!(
        accepted.assumptions.is_empty(),
        "branch-local premise is discharged"
    );

    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&23_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(23))
    );
    let mut unknown = bytes.clone();
    unknown[34] = 27;
    assert_eq!(
        decode_proof_bundle(&unknown),
        Err(ProofCodecError::InvalidTag("ProofRule", 27))
    );
    let mut excessive_count = bytes.clone();
    assert_eq!(&excessive_count[50..54], &2_u32.to_le_bytes());
    excessive_count[50..54].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(decode_proof_bundle(&excessive_count).is_err());
    for length in 0..bytes.len() {
        assert!(
            decode_proof_bundle(&bytes[..length]).is_err(),
            "truncation at {length}"
        );
    }
}

#[test]
fn decoded_missing_and_swapped_branches_are_not_logical_evidence() {
    for swapped in [false, true] {
        let mut proof = proof();
        let ProofRule::DisjunctionElimination { branches, .. } = &mut proof.rule else {
            unreachable!()
        };
        if swapped {
            branches.swap(0, 1);
        } else {
            branches.pop();
        }
        let bytes =
            encode_proof_bundle(&bundle(proof)).expect("logical errors are structurally encodable");
        let proof = decoded_proof(&bytes);
        assert_eq!(
            check_certificate(
                &PropositionContext::default(),
                &Proposition::Truth,
                &[],
                &[],
                &proof
            ),
            Err(if swapped {
                ProofError::AssumptionConclusionMismatch(0)
            } else {
                ProofError::DisjunctionArityMismatch
            }),
        );
    }
}

#[test]
fn decoding_counts_both_premise_and_branch_nesting_before_descent() {
    let shallow = encode_proof_bundle(&bundle(truth())).expect("shallow envelope");
    let truth_bytes = &shallow[33..36];
    for nest_disjunction in [false, true] {
        for depth in [256, 257] {
            let mut tree = truth_bytes.to_vec();
            for _ in 0..depth {
                // A truth conclusion followed by rule 16. Logical premise
                // shapes are deliberately irrelevant to codec depth custody.
                let mut wrapper = vec![truth_bytes[0], 16];
                if nest_disjunction {
                    wrapper.extend_from_slice(&tree);
                    wrapper.extend_from_slice(&0_u32.to_le_bytes());
                } else {
                    wrapper.extend_from_slice(truth_bytes);
                    wrapper.extend_from_slice(&1_u32.to_le_bytes());
                    wrapper.extend_from_slice(&tree);
                }
                tree = wrapper;
            }
            let mut bytes = shallow[..33].to_vec();
            bytes.extend_from_slice(&tree);
            bytes.extend_from_slice(&shallow[36..]);
            if depth == 256 {
                let decoded = decode_proof_bundle(&bytes).expect("exact limit");
                assert_eq!(encode_proof_bundle(&decoded), Ok(bytes));
            } else {
                assert_eq!(
                    decode_proof_bundle(&bytes),
                    Err(ProofCodecError::ProofNestingTooDeep)
                );
            }
        }
    }
}
