use super::{
    canonical_artifact, kernel_bundle, proof_recursive_component, proof_recursive_evidence,
    representative_bundle, semantic_module,
};
use crate::artifact::{
    evidence_id, obligation_id, place_id, structural_case_id, structural_field_id, value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, CorrelatedAffineBranchWitness,
    CorrelatedAffineStepWitness, EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, RecursiveComponentCertificate, RecursiveEdgeCertificate,
};
use semantic_vocabulary::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatFormat,
    IeeeFloatStructuralField, IntegerSign, IntegerType, IntegerValue, Proposition,
    PropositionContext, PropositionError, RankingRelationId, RecursiveComponentId, ScalarTerm,
    ScalarType,
};
use terminal_codec::{
    CanonicalTerminalArtifact, ProofCodecError, decode_module, decode_proof_bundle,
    encode_proof_bundle, render_verified_proof_synopsis, terminal_psi_identity,
};
use terminal_verifier::{
    ObligationEvidence, ProofBundle, RecursiveComponentEvidence, verify_module,
};

#[test]
fn proof_bundle_uses_one_current_canonical_vocabulary() {
    let bundle = representative_bundle();
    let bytes = encode_proof_bundle(&bundle).expect("representative proof bundle should encode");

    assert_eq!(&bytes[..8], b"PSIPRF\0\0");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut noncanonical = bytes.clone();
    noncanonical[14..22].copy_from_slice(&3_u64.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&noncanonical),
        Err(ProofCodecError::NonCanonicalEvidenceOrder)
    );

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        decode_proof_bundle(&trailing),
        Err(ProofCodecError::TrailingBytes(1))
    );
    let mut stale = bytes;
    stale[8..10].copy_from_slice(&26_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(26))
    );
}

#[test]
fn grouped_recursive_component_evidence_round_trips_and_rejects_reordering() {
    let route = |identity| {
        EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: evidence_id(identity),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        })
    };
    let component = |component_raw, certificate_raw, edge_base| RecursiveComponentEvidence {
        component: RecursiveComponentId::new(component_raw).unwrap(),
        certificate: RecursiveComponentCertificate {
            identity: evidence_id(certificate_raw),
            ranking_relation: RankingRelationId::new(77).unwrap(),
            well_foundedness: route(certificate_raw + 1),
            edges: vec![
                RecursiveEdgeCertificate {
                    obligation: obligation_id(edge_base),
                    evidence: route(certificate_raw + 2),
                },
                RecursiveEdgeCertificate {
                    obligation: obligation_id(edge_base + 1),
                    evidence: route(certificate_raw + 3),
                },
            ],
        },
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        evidence: Vec::new(),
        recursive_components: vec![component(301, 401, 501), component(302, 411, 511)],
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
    };
    let bytes = encode_proof_bundle(&bundle).expect("grouped recursion evidence encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut reordered = bundle;
    reordered.recursive_components.reverse();
    assert_eq!(
        encode_proof_bundle(&reordered),
        Err(ProofCodecError::NonCanonicalRecursiveComponentEvidence),
    );
}

#[test]
fn verified_synopsis_reports_one_shared_well_foundedness_and_every_exact_recursive_call() {
    let mut module = semantic_module();
    module.proof_recursive_components = vec![proof_recursive_component()];
    let mut bundle = kernel_bundle();
    bundle.recursive_components = proof_recursive_evidence(&module);
    let verified = verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("recursive proof bundle should verify before synopsis rendering");
    let synopsis = render_verified_proof_synopsis(&verified).expect("verified synopsis");

    assert_eq!(synopsis.matches("recursive-component ").count(), 1);
    assert_eq!(synopsis.matches("  member ").count(), 2);
    assert_eq!(synopsis.matches("  well-founded obligation ").count(), 1);
    assert_eq!(synopsis.matches("  decrease obligation ").count(), 4);
    assert!(synopsis.contains("package::left::step"));
    assert!(synopsis.contains("package::right::step"));

    let mut stale = module;
    stale.proof_recursive_components[0].edges[0].strict_member_path =
        vec!["package::Node::right".into()];
    assert!(verify_module(&stale, &bundle, &AdmissionProfile::default()).is_err());
}

#[test]
fn canonical_terminal_artifact_owns_and_replays_exact_sections() {
    let module = semantic_module();
    let proof = kernel_bundle();
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("representative Terminal module verifies");

    let artifact = canonical_artifact(&module, &proof, None);
    artifact
        .validate()
        .expect("canonical artifact independently replays");
    assert_eq!(
        artifact.manifest().semantic(),
        terminal_psi_identity(&module).expect("semantic identity")
    );
    assert_eq!(decode_module(artifact.semantic_bytes()), Ok(module));
    assert_eq!(decode_proof_bundle(artifact.proof_bytes()), Ok(proof));
    assert!(artifact.optimization().selections().is_empty());
    assert_eq!(
        artifact.manifest().optimization(),
        artifact.optimization().identity()
    );
    assert!(artifact.debug_bytes().is_none());
    assert!(artifact.manifest().installation().is_none());
}

#[test]
fn canonical_terminal_artifact_transport_round_trips_without_producer_objects() {
    let artifact = canonical_artifact(&semantic_module(), &kernel_bundle(), None);
    let identity = artifact.manifest().identity();
    let bytes = artifact.to_bytes();
    drop(artifact);

    assert_eq!(&bytes[..8], b"PSIART\0\0");
    assert_eq!(&bytes[8..10], &2_u16.to_le_bytes());
    let decoded = CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("source-free Terminal artifact envelope");
    assert_eq!(decoded.manifest().identity(), identity);
    assert_eq!(decoded.to_bytes(), bytes);

    let semantic_len = usize::try_from(u64::from_le_bytes(bytes[10..18].try_into().unwrap()))
        .expect("semantic section length");
    let proof_len = usize::try_from(u64::from_le_bytes(bytes[18..26].try_into().unwrap()))
        .expect("proof section length");
    let optimization_start = 35 + semantic_len + proof_len;
    let mut corrupt_optimization = bytes.clone();
    corrupt_optimization[optimization_start] ^= 1;
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&corrupt_optimization),
        Err(
            terminal_codec::CanonicalTerminalArtifactError::Optimization(
                terminal_codec::PsiOptimizationExecutionRecordDecodeError::InvalidMagic
            )
        )
    ));

    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 1;
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&wrong_magic),
        Err(terminal_codec::CanonicalTerminalArtifactError::Envelope(
            terminal_codec::CanonicalTerminalArtifactEnvelopeError::InvalidMagic
        ))
    ));

    let mut stale = bytes.clone();
    stale[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&stale),
        Err(terminal_codec::CanonicalTerminalArtifactError::Envelope(
            terminal_codec::CanonicalTerminalArtifactEnvelopeError::UnsupportedFormatMarker(1)
        ))
    ));

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&trailing),
        Err(terminal_codec::CanonicalTerminalArtifactError::Envelope(
            terminal_codec::CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1)
        ))
    ));
    assert!(CanonicalTerminalArtifact::from_bytes(&bytes[..bytes.len() - 1]).is_err());
}

#[test]
fn proof_format_round_trips_terminal_proposition_disjunction() {
    let conclusion = Proposition::Disjunction(vec![Proposition::Truth, Proposition::Falsehood]);
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Assumption { index: 0 },
            }),
            index: 0,
        },
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(71),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(71),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };
    let accepted = proof_admission::accept_certificate(
        &PropositionContext::default(),
        &conclusion,
        &[Proposition::Truth],
        &[],
        &proof,
    )
    .expect("left disjunct certificate");
    assert_eq!(
        accepted.rules,
        vec![
            proof_admission::AcceptedProofRule::Assumption,
            proof_admission::AcceptedProofRule::DisjunctionIntroduction,
        ]
    );
    assert_eq!(accepted.assumptions.len(), 1);
    assert_eq!(accepted.assumptions[0].index, 0);
    assert_eq!(accepted.assumptions[0].proposition, Proposition::Truth);

    let bytes = encode_proof_bundle(&bundle).expect("disjunction proof bytes encode");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[40], 9, "canonical disjunction-introduction tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut corrupt_tag = bytes.clone();
    corrupt_tag[40] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );

    let mut corrupt_index = bytes.clone();
    corrupt_index[47..51].copy_from_slice(&2_u32.to_le_bytes());
    let decoded = decode_proof_bundle(&corrupt_index)
        .expect("an out-of-range logical index remains syntactically canonical");
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("fixture carries a certificate")
    };
    assert_eq!(
        proof_admission::check_certificate(
            &PropositionContext::default(),
            &conclusion,
            &[Proposition::Truth],
            &[],
            &certificate.proof,
        ),
        Err(proof_admission::ProofError::UnknownDisjunct(2))
    );

    let mut stale = bytes;
    stale[8..10].copy_from_slice(&14_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(14))
    );
}

#[test]
fn proof_format_assigns_tag_ten_to_integer_order_transitivity() {
    let child = || ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(72),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(72),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(child()),
                        middle_less_or_equal_right: Box::new(child()),
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("integer-order proof node encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 10, "canonical integer-order-transitivity tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn proof_format_assigns_tag_eleven_to_integer_order_substitution() {
    let child = || ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let wire_bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(74),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(74),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(child()),
                        equality: Box::new(child()),
                        endpoint: 1,
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&wire_bundle).expect("integer substitution node encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 11, "canonical integer-order-substitution tag");
    assert_eq!(&bytes[41..45], &1_u32.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(wire_bundle));

    let mut corrupt_tag = bytes.clone();
    corrupt_tag[34] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );

    let mut stale = bytes;
    stale[8..10].copy_from_slice(&14_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(14))
    );

    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = ScalarTerm::value(value_id(75), ScalarType::Integer(integer));
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let relation = Proposition::LessOrEqual(literal(1), literal(5));
    let equality = Proposition::Equal(literal(5), divisor.clone());
    let conclusion = Proposition::LessOrEqual(literal(1), divisor);
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(75),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(75),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerOrderSubstitution {
                        relation: Box::new(ProofNode {
                            conclusion: relation,
                            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                        }),
                        equality: Box::new(ProofNode {
                            conclusion: equality.clone(),
                            rule: ProofRule::SemanticAxiom { index: 0 },
                        }),
                        endpoint: 1,
                    },
                },
            }),
        }],
    };
    let mut corrupt_endpoint =
        encode_proof_bundle(&bundle).expect("checked substitution certificate encodes");
    // Recursive components, control cycles, crash obligations and evidence
    // producers each end the bundle as an empty count in this fixture.
    let trailing_empty_section_counts = 4 * std::mem::size_of::<u32>();
    let endpoint =
        corrupt_endpoint.len() - trailing_empty_section_counts - std::mem::size_of::<u32>();
    corrupt_endpoint[endpoint..endpoint + 4].copy_from_slice(&2_u32.to_le_bytes());
    let decoded = decode_proof_bundle(&corrupt_endpoint)
        .expect("an out-of-range endpoint remains syntactically canonical");
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("fixture carries a certificate")
    };
    let context =
        PropositionContext::from_value_types([(value_id(75), ScalarType::Integer(integer))])
            .expect("context");
    assert_eq!(
        proof_admission::check_certificate(
            &context,
            &conclusion,
            &[],
            &[equality],
            &certificate.proof,
        ),
        Err(proof_admission::ProofError::UnknownIntegerOrderEndpoint(2))
    );
}

#[test]
fn proof_format_assigns_tag_twelve_to_integer_affine_bound() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = ScalarTerm::value(value_id(76), ScalarType::Integer(integer));
    let target = ScalarTerm::value(value_id(77), ScalarType::Integer(integer));
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(76),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(76),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                        }),
                        witness: IntegerAffineWitness {
                            root,
                            target,
                            definition_axioms: vec![2, 5],
                            literal_axioms: vec![None, Some(4)],
                        },
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("integer affine bound node encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 12, "canonical integer-affine-bound tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let literal_tags = bytes
        .windows(10)
        .position(|window| window == [0, 1, 4, 0, 0, 0, 0, 0, 0, 0])
        .expect("aligned None/Some(4) literal custody encoding");
    let mut corrupt_literal_tag = bytes.clone();
    corrupt_literal_tag[literal_tags + 1] = 2;
    assert_eq!(
        decode_proof_bundle(&corrupt_literal_tag),
        Err(ProofCodecError::UnknownIntegerAffineLiteralTag(2)),
    );

    let mut old_format = bytes.clone();
    old_format[8..10].copy_from_slice(&19_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&old_format),
        Err(ProofCodecError::UnsupportedFormatMarker(19)),
    );

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn proof_format_assigns_tag_thirteen_to_integer_cast_chain_bound() {
    let source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(77),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(77),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerCastBound {
                        root_bound: Box::new(ProofNode {
                            conclusion: Proposition::Truth,
                            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                        }),
                        witness: IntegerCastChainWitness {
                            root: ScalarTerm::value(value_id(78), ScalarType::Integer(source)),
                            target: ScalarTerm::value(value_id(79), ScalarType::Integer(target)),
                            definition_axioms: vec![3, 8],
                        },
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("integer cast bound node encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 13, "canonical integer-cast-bound tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn proof_format_assigns_tag_fourteen_to_integer_correlated_forbidden_roots() {
    let integer = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let root = ScalarTerm::value(value_id(80), ScalarType::Integer(integer));
    let dividend = ScalarTerm::value(value_id(81), ScalarType::Integer(integer));
    let divisor = ScalarTerm::value(value_id(82), ScalarType::Integer(integer));
    let conclusion = Proposition::Truth;
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(80),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(80),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerCorrelatedForbiddenRoots {
                        witness: IntegerCorrelatedForbiddenRootWitness {
                            dividend: CorrelatedAffineBranchWitness {
                                root: root.clone(),
                                target: dividend,
                                steps: vec![CorrelatedAffineStepWitness {
                                    definition_axiom: 1,
                                    literal_axiom: Some(2),
                                }],
                            },
                            divisor: CorrelatedAffineBranchWitness {
                                root,
                                target: divisor,
                                steps: vec![CorrelatedAffineStepWitness {
                                    definition_axiom: 3,
                                    literal_axiom: None,
                                }],
                            },
                            definition_axiom_count: 4,
                            lower_bound_axiom: 4,
                            upper_bound_axiom: 5,
                            conclusion,
                        },
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("integer correlated proof node encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(bytes[34], 14, "canonical correlated-forbidden-roots tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let literal_tag = bytes
        .windows(9)
        .position(|window| window == [1, 0, 0, 0, 1, 2, 0, 0, 0])
        .expect("correlated affine Some(2) literal custody encoding")
        + 4;
    let mut corrupt_literal_tag = bytes.clone();
    corrupt_literal_tag[literal_tag] = 2;
    assert_eq!(
        decode_proof_bundle(&corrupt_literal_tag),
        Err(ProofCodecError::UnknownIntegerCorrelatedAffineLiteralTag(2)),
    );

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 24;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 24))
    );
}

#[test]
fn proof_format_round_trips_negative_nonzero_certificate() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let divisor = ScalarTerm::value(value_id(73), ScalarType::Integer(integer));
    let literal =
        |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
    let tighter = Proposition::LessOrEqual(divisor.clone(), literal(-2));
    let negative = Proposition::LessOrEqual(divisor.clone(), literal(-1));
    let goal = Proposition::Disjunction(vec![
        negative.clone(),
        Proposition::LessOrEqual(literal(1), divisor),
    ]);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(ProofNode {
                conclusion: negative,
                rule: ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(ProofNode {
                        conclusion: tighter.clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                    middle_less_or_equal_right: Box::new(ProofNode {
                        conclusion: Proposition::LessOrEqual(literal(-2), literal(-1)),
                        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                    }),
                },
            }),
            index: 0,
        },
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(73),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(73),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };
    let context =
        PropositionContext::from_value_types([(value_id(73), ScalarType::Integer(integer))])
            .expect("context");
    proof_admission::check_certificate(
        &context,
        &goal,
        &[],
        std::slice::from_ref(&tighter),
        &proof,
    )
    .expect("negative nonzero certificate checks before encoding");

    let bytes = encode_proof_bundle(&bundle).expect("negative nonzero proof encodes");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    let decoded = decode_proof_bundle(&bytes).expect("negative nonzero proof decodes");
    assert_eq!(decoded, bundle);
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("fixture carries a certificate")
    };
    proof_admission::check_certificate(&context, &goal, &[], &[tighter], &certificate.proof)
        .expect("negative nonzero certificate checks after decoding");
}

#[test]
fn proof_format_round_trips_atomic_ieee_structural_equality() {
    let left = IeeeFloatStructuralField::new(
        place_id(1),
        vec![
            CanonicalStructuralPathSegment::Case(structural_case_id(1)),
            CanonicalStructuralPathSegment::Field(structural_field_id(1)),
        ],
    )
    .expect("left IEEE field");
    let right = IeeeFloatStructuralField::new(
        place_id(2),
        vec![
            CanonicalStructuralPathSegment::Case(structural_case_id(1)),
            CanonicalStructuralPathSegment::Field(structural_field_id(1)),
        ],
    )
    .expect("right IEEE field");
    let conclusion = Proposition::IeeeFloatComparison {
        kind: semantic_vocabulary::IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary32,
        left,
        right,
    };
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(72),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(72),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let bytes = encode_proof_bundle(&bundle).expect("IEEE proof bytes encode");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut inequality = bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut inequality.evidence[0].route else {
        unreachable!()
    };
    let Proposition::IeeeFloatComparison { kind, .. } = &mut certificate.proof.conclusion else {
        unreachable!()
    };
    *kind = semantic_vocabulary::IeeeFloatComparisonKind::NotEqual;
    let inequality_bytes = encode_proof_bundle(&inequality).expect("IEEE inequality proof bytes");
    assert_eq!(decode_proof_bundle(&inequality_bytes), Ok(inequality));

    let mut noncanonical = bundle;
    let EvidenceRoute::CertificateDerived(certificate) = &mut noncanonical.evidence[0].route else {
        unreachable!()
    };
    let Proposition::IeeeFloatComparison { left, right, .. } = &mut certificate.proof.conclusion
    else {
        unreachable!()
    };
    std::mem::swap(left, right);
    assert_eq!(
        encode_proof_bundle(&noncanonical),
        Err(ProofCodecError::MalformedProposition(
            PropositionError::NonCanonicalIeeeFloatComparisonOperands
        ))
    );
}

#[test]
fn proof_format_round_trips_atomic_byte_sequence_equality() {
    let left = ByteSequenceStructuralField::new(
        place_id(1),
        vec![CanonicalStructuralPathSegment::Field(structural_field_id(
            1,
        ))],
    )
    .expect("left byte-sequence field");
    let right = ByteSequenceStructuralField::new(
        place_id(2),
        vec![CanonicalStructuralPathSegment::Field(structural_field_id(
            1,
        ))],
    )
    .expect("right byte-sequence field");
    let bundle = ProofBundle {
        crash_obligations: Vec::new(),
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(73),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(73),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::ByteSequenceEqual { left, right },
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let bytes = encode_proof_bundle(&bundle).expect("byte-sequence proof bytes encode");
    assert_eq!(&bytes[8..10], &34_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut noncanonical = bundle;
    let EvidenceRoute::CertificateDerived(certificate) = &mut noncanonical.evidence[0].route else {
        unreachable!()
    };
    let Proposition::ByteSequenceEqual { left, right } = &mut certificate.proof.conclusion else {
        unreachable!()
    };
    std::mem::swap(left, right);
    assert_eq!(
        encode_proof_bundle(&noncanonical),
        Err(ProofCodecError::MalformedProposition(
            PropositionError::NonCanonicalByteSequenceEqualOperands
        ))
    );
}
