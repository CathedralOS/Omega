use proof_admission::{
    CertificateEnvelope, EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    PrimitiveJudgment, ProofError, ProofNode, ProofRule, ProofSystemMarker, accept_certificate,
    check_certificate,
};
use semantic_vocabulary::{
    EvidenceIdentity, ObligationId, Proposition, PropositionContext, ScalarTerm,
};
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
        recursive_components: Vec::new(),
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
    assert_eq!(&bytes[8..10], &24_u16.to_le_bytes());
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
    unknown[34] = 17;
    assert_eq!(
        decode_proof_bundle(&unknown),
        Err(ProofCodecError::InvalidTag("ProofRule", 17))
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
fn each_case_child_counts_toward_the_proof_depth_limit() {
    for nest_disjunction in [false, true] {
        let mut nested = truth();
        for _ in 0..258 {
            let (disjunction, branches) = if nest_disjunction {
                (nested, Vec::new())
            } else {
                (truth(), vec![nested])
            };
            nested = ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(disjunction),
                    branches,
                },
            };
        }
        assert_eq!(
            encode_proof_bundle(&bundle(nested)),
            Err(ProofCodecError::ProofNestingTooDeep)
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

#[test]
fn existing_child_order_and_trailing_witness_bytes_are_unchanged() {
    let child = |index| ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Assumption { index },
    };
    let first = || Box::new(child(1));
    let second = || Box::new(child(2));
    let first_bytes = [1, 3, 1, 0, 0, 0];
    let second_bytes = [1, 3, 2, 0, 0, 0];
    let mut fixtures = vec![
        (
            ProofRule::ConjunctionIntroduction(vec![child(1), child(2)]),
            vec![4, 2, 0, 0, 0],
            vec![first_bytes, second_bytes],
            vec![],
        ),
        (
            ProofRule::ConjunctionElimination {
                conjunction: first(),
                conjunct: 9,
            },
            vec![5],
            vec![first_bytes],
            vec![9, 0, 0, 0],
        ),
        (
            ProofRule::ImplicationIntroduction { body: first() },
            vec![6],
            vec![first_bytes],
            vec![],
        ),
        (
            ProofRule::ImplicationElimination {
                implication: first(),
                premise: second(),
            },
            vec![7],
            vec![first_bytes, second_bytes],
            vec![],
        ),
        (
            ProofRule::EqualityTransitivity {
                left_equals_middle: first(),
                middle_equals_right: second(),
            },
            vec![8],
            vec![first_bytes, second_bytes],
            vec![],
        ),
        (
            ProofRule::DisjunctionIntroduction {
                disjunct: first(),
                index: 9,
            },
            vec![9],
            vec![first_bytes],
            vec![9, 0, 0, 0],
        ),
        (
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: first(),
                middle_less_or_equal_right: second(),
            },
            vec![10],
            vec![first_bytes, second_bytes],
            vec![],
        ),
        (
            ProofRule::IntegerLessOrEqualSubstitution {
                relation: first(),
                equality: second(),
                endpoint: 9,
            },
            vec![11],
            vec![first_bytes, second_bytes],
            vec![9, 0, 0, 0],
        ),
        (
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound: first(),
                right_bound: second(),
                definition_axiom: 9,
            },
            vec![15],
            vec![first_bytes, second_bytes],
            vec![9, 0, 0, 0],
        ),
    ];
    // Witness scalar shapes need only be structurally encodable here; kernel
    // validity is deliberately separate from preserving the proof grammar.
    fixtures.push((
        ProofRule::IntegerAffineBound {
            root_bound: first(),
            witness: IntegerAffineWitness {
                root: ScalarTerm::boolean(false),
                target: ScalarTerm::boolean(true),
                definition_axioms: vec![9],
                literal_axioms: vec![Some(7)],
            },
        },
        vec![12],
        vec![first_bytes],
        vec![
            2, 0, 2, 1, 1, 0, 0, 0, 9, 0, 0, 0, 1, 0, 0, 0, 1, 7, 0, 0, 0,
        ],
    ));
    fixtures.push((
        ProofRule::IntegerCastBound {
            root_bound: first(),
            witness: IntegerCastChainWitness {
                root: ScalarTerm::boolean(false),
                target: ScalarTerm::boolean(true),
                definition_axioms: vec![9],
            },
        },
        vec![13],
        vec![first_bytes],
        vec![2, 0, 2, 1, 1, 0, 0, 0, 9, 0, 0, 0],
    ));
    for (rule, mut expected, children, suffix) in fixtures {
        for child in children {
            expected.extend_from_slice(&child);
        }
        expected.extend_from_slice(&suffix);
        let bundle = bundle(ProofNode {
            conclusion: Proposition::Truth,
            rule,
        });
        let bytes = encode_proof_bundle(&bundle).expect("existing proof rule bytes");
        assert_eq!(
            &bytes[34..bytes.len() - 8],
            expected,
            "fixed wire tag {}",
            expected[0]
        );
        assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
    }
}

#[test]
fn accepted_depth_case_proof_is_kernel_checked_on_the_default_stack() {
    let mut nested = truth();
    for _ in 0..255 {
        nested = ProofNode {
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
                branches: vec![nested, truth()],
            },
        };
    }
    // The last disjunction introduction's child is exactly depth 256.
    let bytes = encode_proof_bundle(&bundle(nested)).expect("accepted depth");
    let decoded = decode_proof_bundle(&bytes).expect("canonical case proof");
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("certificate fixture")
    };
    check_certificate(
        &PropositionContext::default(),
        &Proposition::Truth,
        &[],
        &[],
        &certificate.proof,
    )
    .expect("logical proof checks at the accepted codec limit");
}

#[test]
fn existing_recursive_rules_remain_kernel_safe_at_the_codec_limit() {
    for implication in [false, true] {
        let mut nested = truth();
        for _ in 0..128 {
            nested = if implication {
                ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::ImplicationElimination {
                        implication: Box::new(ProofNode {
                            conclusion: Proposition::Implication {
                                premise: Box::new(Proposition::Truth),
                                conclusion: Box::new(Proposition::Truth),
                            },
                            rule: ProofRule::ImplicationIntroduction {
                                body: Box::new(nested),
                            },
                        }),
                        premise: Box::new(truth()),
                    },
                }
            } else {
                ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(ProofNode {
                            conclusion: Proposition::Conjunction(vec![
                                Proposition::Truth,
                                Proposition::Truth,
                            ]),
                            rule: ProofRule::ConjunctionIntroduction(vec![nested, truth()]),
                        }),
                        conjunct: 0,
                    },
                }
            };
        }
        let bytes = encode_proof_bundle(&bundle(nested)).expect("old rule at accepted depth");
        let decoded = decode_proof_bundle(&bytes).expect("old rule bytes");
        let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
            unreachable!("certificate fixture")
        };
        let accepted = accept_certificate(
            &PropositionContext::default(),
            &Proposition::Truth,
            &[],
            &[],
            &certificate.proof,
        )
        .expect("old recursive rules verify on the default stack");
        assert!(accepted.assumptions.is_empty());
    }
}
