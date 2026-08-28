use psi_core::{
    AdmissionSiteId, BlockId, ByteSequenceStructuralField, CanonicalStructuralPathSegment,
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, EvidenceIdentity, IeeeFloatFormat, IeeeFloatStructuralField, IntegerSign,
    IntegerType, IntegerValue, MachineId, ObligationId, OperationId, PlaceId, ProfileDecisionId,
    Proposition, PropositionContext, PropositionError, ScalarTerm, ScalarType, StructuralCaseId,
    StructuralCaseSubject, StructuralFieldId, StructuralPlaceKind, ValueId,
};
use psi_proof_admission::{
    AdmissionEvidence, AdmissionKind, AdmissionProfile, CertificateEnvelope, EvidenceRoute,
    IntegerAffineWitness, IntegerCastChainWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{
    ArtifactManifestError, CanonicalTerminalArtifact, ProofCodecError, build_artifact_manifest,
    current_rust_operation_semantics_trust_identity, current_terminal_trust_graph, decode_module,
    decode_proof_bundle, encode_proof_bundle, proof_bundle_fingerprint,
    render_verified_proof_synopsis, terminal_psi_identity, validate_artifact_manifest,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn proof_bundle_uses_one_current_canonical_vocabulary() {
    let bundle = representative_bundle();
    let bytes = encode_proof_bundle(&bundle).expect("representative proof bundle should encode");

    assert_eq!(&bytes[..8], b"PSIPRF\0\0");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
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
    stale[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedFormatMarker(1))
    );
}

#[test]
fn canonical_terminal_artifact_owns_and_replays_exact_sections() {
    let module = semantic_module();
    let proof = kernel_bundle();
    verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("representative Terminal module verifies");

    let artifact = CanonicalTerminalArtifact::from_parts(&module, &proof, None)
        .expect("canonical Terminal artifact");
    artifact
        .validate()
        .expect("canonical artifact independently replays");
    assert_eq!(
        artifact.manifest().semantic(),
        terminal_psi_identity(&module).expect("semantic identity")
    );
    assert_eq!(decode_module(artifact.semantic_bytes()), Ok(module));
    assert_eq!(decode_proof_bundle(artifact.proof_bytes()), Ok(proof));
    assert!(artifact.debug_bytes().is_none());
    assert!(artifact.manifest().installation().is_none());
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
    let accepted = psi_proof_admission::accept_certificate(
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
            psi_proof_admission::AcceptedProofRule::Assumption,
            psi_proof_admission::AcceptedProofRule::DisjunctionIntroduction,
        ]
    );
    assert_eq!(accepted.assumptions.len(), 1);
    assert_eq!(accepted.assumptions[0].index, 0);
    assert_eq!(accepted.assumptions[0].proposition, Proposition::Truth);

    let bytes = encode_proof_bundle(&bundle).expect("disjunction proof bytes encode");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(bytes[40], 9, "canonical disjunction-introduction tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut corrupt_tag = bytes.clone();
    corrupt_tag[40] = 14;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 14))
    );

    let mut corrupt_index = bytes.clone();
    corrupt_index[47..51].copy_from_slice(&2_u32.to_le_bytes());
    let decoded = decode_proof_bundle(&corrupt_index)
        .expect("an out-of-range logical index remains syntactically canonical");
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("fixture carries a certificate")
    };
    assert_eq!(
        psi_proof_admission::check_certificate(
            &PropositionContext::default(),
            &conclusion,
            &[Proposition::Truth],
            &[],
            &certificate.proof,
        ),
        Err(psi_proof_admission::ProofError::UnknownDisjunct(2))
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
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(bytes[34], 10, "canonical integer-order-transitivity tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 14;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 14))
    );
}

#[test]
fn proof_format_assigns_tag_eleven_to_integer_order_substitution() {
    let child = || ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let wire_bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(74),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(74),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(child()),
                        equality: Box::new(child()),
                        endpoint: 1,
                    },
                },
            }),
        }],
    };

    let bytes = encode_proof_bundle(&wire_bundle).expect("integer substitution node encodes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(bytes[34], 11, "canonical integer-order-substitution tag");
    assert_eq!(&bytes[41..45], &1_u32.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(wire_bundle));

    let mut corrupt_tag = bytes.clone();
    corrupt_tag[34] = 14;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 14))
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
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(75),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(75),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: conclusion.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
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
    let endpoint = corrupt_endpoint.len() - 8;
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
        psi_proof_admission::check_certificate(
            &context,
            &conclusion,
            &[],
            &[equality],
            &certificate.proof,
        ),
        Err(psi_proof_admission::ProofError::UnknownIntegerOrderEndpoint(2))
    );
}

#[test]
fn proof_format_assigns_tag_twelve_to_integer_affine_bound() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = ScalarTerm::value(value_id(76), ScalarType::Integer(integer));
    let target = ScalarTerm::value(value_id(77), ScalarType::Integer(integer));
    let bundle = ProofBundle {
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
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
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
    old_format[8..10].copy_from_slice(&18_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&old_format),
        Err(ProofCodecError::UnsupportedFormatMarker(18)),
    );

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 14;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 14))
    );
}

#[test]
fn proof_format_assigns_tag_thirteen_to_integer_cast_chain_bound() {
    let source = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let target = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let bundle = ProofBundle {
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
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(bytes[34], 13, "canonical integer-cast-bound tag");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));

    let mut corrupt_tag = bytes;
    corrupt_tag[34] = 14;
    assert_eq!(
        decode_proof_bundle(&corrupt_tag),
        Err(ProofCodecError::InvalidTag("ProofRule", 14))
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
    psi_proof_admission::check_certificate(
        &context,
        &goal,
        &[],
        std::slice::from_ref(&tighter),
        &proof,
    )
    .expect("negative nonzero certificate checks before encoding");

    let bytes = encode_proof_bundle(&bundle).expect("negative nonzero proof encodes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    let decoded = decode_proof_bundle(&bytes).expect("negative nonzero proof decodes");
    assert_eq!(decoded, bundle);
    let EvidenceRoute::CertificateDerived(certificate) = &decoded.evidence[0].route else {
        unreachable!("fixture carries a certificate")
    };
    psi_proof_admission::check_certificate(&context, &goal, &[], &[tighter], &certificate.proof)
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
        kind: psi_core::IeeeFloatComparisonKind::Equal,
        format: IeeeFloatFormat::Binary32,
        left,
        right,
    };
    let bundle = ProofBundle {
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
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut inequality = bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut inequality.evidence[0].route else {
        unreachable!()
    };
    let Proposition::IeeeFloatComparison { kind, .. } = &mut certificate.proof.conclusion else {
        unreachable!()
    };
    *kind = psi_core::IeeeFloatComparisonKind::NotEqual;
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
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
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

#[test]
fn proof_format_round_trips_structural_case_membership() {
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(74),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(74),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::StructuralCaseMembership {
                        subject: StructuralCaseSubject::new(
                            place_id(1),
                            vec![CanonicalStructuralPathSegment::Field(structural_field_id(
                                2,
                            ))],
                        ),
                        case: structural_case_id(3),
                    },
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let bytes = encode_proof_bundle(&bundle).expect("case-membership proof bytes encode");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn synopsis_is_projected_from_the_exact_accepted_certificate() {
    let module = semantic_module();
    let primitive_bundle = certificate_bundle();
    let primitive_verified =
        verify_module(&module, &primitive_bundle, &AdmissionProfile::default())
            .expect("primitive certificate");
    let first = render_verified_proof_synopsis(&primitive_verified).expect("primitive synopsis");
    assert_eq!(
        first,
        render_verified_proof_synopsis(&primitive_verified).expect("deterministic synopsis")
    );
    assert!(first.starts_with("proof-bundle "));
    assert!(first.contains("obligation 1 goal "));
    assert!(first.contains("certificate 9 proof-system 1"));
    assert!(first.contains("rule Primitive"));
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    assert!(first.contains(&format!(
        "trust-graph {} entry closure:terminal-pcc-current fully-derived false",
        trust_graph.identity()
    )));
    assert!(first.contains("trust-node implementation:rust-terminal-decoder"));
    assert!(first.contains("trust-node implementation:rust-terminal-verifier"));
    assert!(first.contains("trust-node reduction:integer-conversion"));
    assert!(first.contains("trust-node schema:operation:exact-integer-add"));
    assert_eq!(
        current_rust_operation_semantics_trust_identity(&OperationKind::ExactIntegerAdd {
            left: value_id(1),
            right: value_id(2),
            obligation: obligation_id(3),
        }),
        "schema:operation:exact-integer-add"
    );
    assert_eq!(
        current_rust_operation_semantics_trust_identity(&OperationKind::Call {
            callee: machine_id(9),
            arguments: vec![value_id(1)],
            requirement_obligations: vec![obligation_id(2)],
            crash_continuations: Vec::new(),
        }),
        "algebra:call:call"
    );
    assert!(first.contains("trust-node algebra:call:call"));

    let goal = module.machines[0].contract.ensures[0].proposition.clone();
    let assumption_bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(9),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Assumption { index: 0 },
                },
            }),
        }],
    };
    let assumption_verified =
        verify_module(&module, &assumption_bundle, &AdmissionProfile::default())
            .expect("assumption certificate");
    let second = render_verified_proof_synopsis(&assumption_verified).expect("assumption synopsis");
    assert_ne!(first.lines().next(), second.lines().next());
    assert!(second.contains("rule Assumption"));
    assert!(second.contains("assumption[0]"));
}

#[test]
fn proof_format_canonically_encodes_boolean_equality() {
    let equality =
        ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)).unwrap();
    let goal = Proposition::Equal(equality.clone(), equality);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(101),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(101),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive Boolean-equality certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut stale = bytes;
    stale[31..33].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedProofSystemMarker(2))
    );
}

#[test]
fn proof_format_round_trips_nested_boolean_field_paths() {
    let field = ScalarTerm::boolean_field_path(
        place_id(4),
        vec![
            CanonicalStructuralPathSegment::FixedIndex(3),
            CanonicalStructuralPathSegment::Field(structural_field_id(7)),
            CanonicalStructuralPathSegment::Field(structural_field_id(11)),
        ],
    );
    let goal = Proposition::Equal(field.clone(), field);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(9),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                },
            }),
        }],
    };
    let bytes = encode_proof_bundle(&bundle).expect("Boolean field proof encodes");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    let Proposition::Equal(
        ScalarTerm::BooleanField {
            path: left_path, ..
        },
        ScalarTerm::BooleanField {
            path: right_path, ..
        },
    ) = &mut certificate.proof.conclusion
    else {
        unreachable!()
    };
    left_path[0] = CanonicalStructuralPathSegment::FixedIndex(4);
    right_path[0] = CanonicalStructuralPathSegment::FixedIndex(4);
    let changed_bytes = encode_proof_bundle(&changed).expect("changed fixed index encodes");
    assert_ne!(bytes, changed_bytes);
    assert_ne!(
        proof_bundle_fingerprint(&bundle).unwrap(),
        proof_bundle_fingerprint(&changed).unwrap(),
        "the exact literal index participates in canonical proof identity",
    );
}

#[test]
fn proof_format_round_trips_typed_integer_field_paths() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let field = ScalarTerm::integer_field_path(
        place_id(5),
        vec![
            CanonicalStructuralPathSegment::Field(structural_field_id(13)),
            CanonicalStructuralPathSegment::FixedIndex(2),
            CanonicalStructuralPathSegment::Field(structural_field_id(17)),
        ],
        integer_type,
    );
    let goal = Proposition::Equal(field.clone(), field);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(10),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
                },
            }),
        }],
    };
    let bytes = encode_proof_bundle(&bundle).expect("integer field proof encodes");
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut changed = bundle.clone();
    let EvidenceRoute::CertificateDerived(certificate) = &mut changed.evidence[0].route else {
        unreachable!()
    };
    let Proposition::Equal(
        ScalarTerm::IntegerField {
            path: left_path, ..
        },
        ScalarTerm::IntegerField {
            path: right_path, ..
        },
    ) = &mut certificate.proof.conclusion
    else {
        unreachable!()
    };
    left_path[1] = CanonicalStructuralPathSegment::FixedIndex(3);
    right_path[1] = CanonicalStructuralPathSegment::FixedIndex(3);
    assert_ne!(
        proof_bundle_fingerprint(&bundle).unwrap(),
        proof_bundle_fingerprint(&changed).unwrap(),
        "the exact integer-member path participates in proof identity",
    );
}

#[test]
fn proof_format_canonically_encodes_integer_equality() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(7)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(7)).unwrap();
    let equality = ScalarTerm::integer_equal(integer, left, right).unwrap();
    let goal = Proposition::Equal(equality.clone(), equality);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(102),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(102),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-equality certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_integer_ordering() {
    let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let left = ScalarTerm::integer(integer, IntegerValue::Signed(-1)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Signed(0)).unwrap();
    let ordered = ScalarTerm::integer_less_than(integer, left, right).unwrap();
    let goal = Proposition::Equal(ordered.clone(), ordered);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(103),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(103),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-ordering certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_integer_bitwise_terms() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(0b1100)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(0b1010)).unwrap();
    let bitwise = ScalarTerm::integer_bitwise_and(integer, left, right).unwrap();
    let goal = Proposition::Equal(bitwise.clone(), bitwise);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(104),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(104),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-bitwise certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_wrapping_shift_terms() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let count_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let value = ScalarTerm::integer(value_type, IntegerValue::Unsigned(1)).unwrap();
    let count = ScalarTerm::integer(count_type, IntegerValue::Signed(-1)).unwrap();
    let shifted =
        ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count).unwrap();
    let goal = Proposition::Equal(shifted.clone(), shifted);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(105),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(105),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_integer_bitwise_not() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let operand = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(0x0f)).unwrap();
    let complemented = ScalarTerm::integer_bitwise_not(scalar_type, operand).unwrap();
    let goal = Proposition::Equal(complemented.clone(), complemented);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(106),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(106),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-bitwise-not certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_integer_widening() {
    let source_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let target_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
    let operand = ScalarTerm::integer(source_type, IntegerValue::Signed(-128)).unwrap();
    let widened = ScalarTerm::integer_widen(source_type, target_type, operand).unwrap();
    let goal = Proposition::Equal(widened.clone(), widened);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(107),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(107),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-widen certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_address_carriers() {
    let address = IntegerType::address(64).expect("addr");
    let value = ScalarTerm::integer(address, IntegerValue::Unsigned(0x1234)).unwrap();
    let goal = Proposition::Equal(value.clone(), value);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(108),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(108),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive address certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_right_shifts() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = ScalarTerm::integer(value_type, IntegerValue::Unsigned(1 << 63)).unwrap();
    let count = ScalarTerm::integer(count_type, IntegerValue::Unsigned(63)).unwrap();
    let shifted =
        ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count).unwrap();
    let goal = Proposition::Equal(shifted.clone(), shifted);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(110),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(110),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-right-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_left_shifts() {
    let value_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let count_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let value = ScalarTerm::integer(value_type, IntegerValue::Unsigned(1)).unwrap();
    let count = ScalarTerm::integer(count_type, IntegerValue::Unsigned(31)).unwrap();
    let shifted =
        ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count).unwrap();
    let goal = Proposition::Equal(shifted.clone(), shifted);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(111),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(111),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-left-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_integer_addition() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(40)).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(2)).unwrap();
    let sum = ScalarTerm::exact_integer_add(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(sum.clone(), sum);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(112),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(112),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-add certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_integer_subtraction() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(42)).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(2)).unwrap();
    let difference = ScalarTerm::exact_integer_subtract(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(difference.clone(), difference);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(113),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(113),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-subtract certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_integer_multiplication() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(21)).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(2)).unwrap();
    let product = ScalarTerm::exact_integer_multiply(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(product.clone(), product);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(114),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(114),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-multiply certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_integer_division() {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).expect("u32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(42)).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Unsigned(2)).unwrap();
    let quotient = ScalarTerm::exact_integer_divide(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(quotient.clone(), quotient);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(115),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(115),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_exact_integer_remainder() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Signed(-43)).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Signed(5)).unwrap();
    let remainder = ScalarTerm::exact_integer_remainder(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(remainder.clone(), remainder);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(116),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(116),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_wrapping_integer_division() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Signed(i32::MIN.into())).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Signed(-1)).unwrap();
    let quotient = ScalarTerm::wrapping_integer_divide(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(quotient.clone(), quotient);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(117),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(117),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_wrapping_integer_remainder() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Signed(i32::MIN.into())).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Signed(-1)).unwrap();
    let remainder = ScalarTerm::wrapping_integer_remainder(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(remainder.clone(), remainder);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(118),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(118),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_saturating_integer_division() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Signed(i32::MIN.into())).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Signed(-1)).unwrap();
    let quotient = ScalarTerm::saturating_integer_divide(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(quotient.clone(), quotient);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(119),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(119),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive saturating-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_saturating_integer_remainder() {
    let scalar_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let left = ScalarTerm::integer(scalar_type, IntegerValue::Signed(i32::MIN.into())).unwrap();
    let right = ScalarTerm::integer(scalar_type, IntegerValue::Signed(-1)).unwrap();
    let remainder = ScalarTerm::saturating_integer_remainder(scalar_type, left, right).unwrap();
    let goal = Proposition::Equal(remainder.clone(), remainder);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(120),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(120),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive saturating-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle));
}

#[test]
fn proof_format_canonically_encodes_boolean_negation() {
    let negated = ScalarTerm::boolean_not(ScalarTerm::boolean(false)).unwrap();
    let goal = Proposition::Equal(negated.clone(), negated);
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(100),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(100),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };

    psi_proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive Boolean-negation certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_wrapping_arithmetic() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(200)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap();
    let sum = ScalarTerm::wrapping_integer_add(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(44)).unwrap();
    let goal = Proposition::Equal(sum, reduced);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 wrapping addition proves 44");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_content_certificates() {
    let root = PlaceId::new(80).expect("place");
    let term = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(81).expect("domain"),
            projection_fingerprint: 0x8283,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: Vec::new(),
        },
    };
    let goal = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        },
        term.clone(),
        term,
    ));
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(80),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(80),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: proof.clone(),
            }),
        }],
    };
    let context = PropositionContext::from_value_types_and_places(
        [],
        [(
            root,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        )],
    )
    .expect("context");
    psi_proof_admission::check_certificate(&context, &goal, &[], &[], &proof)
        .expect("reflexive content certificate");

    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_saturating_arithmetic() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(200)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap();
    let sum = ScalarTerm::saturating_integer_add(integer, left, right).unwrap();
    let clamped = ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap();
    let goal = Proposition::Equal(sum, clamped);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 saturating addition proves 255");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_wrapping_subtraction() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
    let difference = ScalarTerm::wrapping_integer_subtract(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(251)).unwrap();
    let goal = Proposition::Equal(difference, reduced);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 wrapping subtraction proves 251");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_saturating_subtraction() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
    let difference = ScalarTerm::saturating_integer_subtract(integer, left, right).unwrap();
    let clamped = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
    let goal = Proposition::Equal(difference, clamped);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 saturating subtraction proves zero");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_wrapping_multiplication() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(20)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(13)).unwrap();
    let product = ScalarTerm::wrapping_integer_multiply(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(4)).unwrap();
    let goal = Proposition::Equal(product, reduced);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 wrapping multiplication proves four");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_sum_case_content_certificates() {
    let root = PlaceId::new(90).expect("place");
    let term = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(91).expect("domain"),
            projection_fingerprint: 0x9293,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root,
            segments: vec![
                ContentPlaceSegment::Case("Present".to_owned()),
                ContentPlaceSegment::Field("payload".to_owned()),
            ],
        },
    };
    let goal = Proposition::ContentConservation(ContentConservation::new(
        ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        },
        term.clone(),
        term,
    ));
    let proof = ProofNode {
        conclusion: goal,
        rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
    };
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(90),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(90),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };

    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_closed_saturating_multiplication() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(20)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(13)).unwrap();
    let product = ScalarTerm::saturating_integer_multiply(integer, left, right).unwrap();
    let clamped = ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap();
    let goal = Proposition::Equal(product, clamped);
    let bundle = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_admission::check_certificate(
        &psi_core::PropositionContext::default(),
        &goal,
        &[],
        &[],
        match &bundle.evidence[0].route {
            EvidenceRoute::CertificateDerived(certificate) => &certificate.proof,
            _ => unreachable!("fixture is certificate-derived"),
        },
    )
    .expect("closed u8 saturating multiplication proves 255");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &19_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_evidence_order_and_proof_depth_fail_closed() {
    let mut unordered = representative_bundle();
    unordered.evidence.swap(0, 1);
    assert_eq!(
        encode_proof_bundle(&unordered),
        Err(ProofCodecError::NonCanonicalEvidenceOrder)
    );

    let mut proof = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    for _ in 0..257 {
        proof = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(proof),
                index: 0,
            },
        };
    }
    let too_deep = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    };
    assert_eq!(
        encode_proof_bundle(&too_deep),
        Err(ProofCodecError::ProofNestingTooDeep)
    );

    let mut substitution = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    for _ in 0..257 {
        substitution = ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(substitution),
                equality: Box::new(ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                }),
                endpoint: 0,
            },
        };
    }
    let too_deep_substitution = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: substitution,
            }),
        }],
    };
    assert_eq!(
        encode_proof_bundle(&too_deep_substitution),
        Err(ProofCodecError::ProofNestingTooDeep)
    );

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = || ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let mut term = literal();
    for _ in 0..257 {
        term = ScalarTerm::wrapping_integer_add(integer, term, literal()).unwrap();
    }
    let deep_term = ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: Proposition::Equal(literal(), term),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };
    assert_eq!(
        encode_proof_bundle(&deep_term),
        Err(ProofCodecError::ScalarTermNestingTooDeep)
    );
}

#[test]
fn proof_replacement_and_attached_sections_change_only_their_identities() {
    let module = semantic_module();
    let kernel = kernel_bundle();
    let certificate = certificate_bundle();
    verify_module(&module, &kernel, &AdmissionProfile::default()).unwrap();
    verify_module(&module, &certificate, &AdmissionProfile::default()).unwrap();

    let semantic_identity = terminal_psi_identity(&module).unwrap();
    let first =
        build_artifact_manifest(&module, &kernel, Some(b"provider=A"), Some(b"source-map=A"))
            .unwrap();
    let replacement = build_artifact_manifest(
        &module,
        &certificate,
        Some(b"provider=A"),
        Some(b"source-map=A"),
    )
    .unwrap();
    assert_eq!(first.semantic(), semantic_identity);
    assert_eq!(replacement.semantic(), semantic_identity);
    assert_eq!(first.obligations(), replacement.obligations());
    assert_ne!(first.proof(), replacement.proof());
    assert_ne!(first.identity(), replacement.identity());
    assert_eq!(first.installation(), replacement.installation());
    assert_eq!(first.debug(), replacement.debug());

    let reinstalled =
        build_artifact_manifest(&module, &kernel, Some(b"provider=B"), Some(b"source-map=A"))
            .unwrap();
    assert_eq!(first.semantic(), reinstalled.semantic());
    assert_eq!(first.obligations(), reinstalled.obligations());
    assert_eq!(first.proof(), reinstalled.proof());
    assert_ne!(first.installation(), reinstalled.installation());
    assert_ne!(first.identity(), reinstalled.identity());

    let stripped_debug =
        build_artifact_manifest(&module, &kernel, Some(b"provider=A"), None).unwrap();
    assert_eq!(first.semantic(), stripped_debug.semantic());
    assert_eq!(first.proof(), stripped_debug.proof());
    assert_eq!(first.installation(), stripped_debug.installation());
    assert_ne!(first.debug(), stripped_debug.debug());
    assert_ne!(first.identity(), stripped_debug.identity());

    let empty_debug =
        build_artifact_manifest(&module, &kernel, Some(b"provider=A"), Some(b"")).unwrap();
    assert_ne!(stripped_debug.debug(), empty_debug.debug());
    assert_ne!(stripped_debug.identity(), empty_debug.identity());

    let equal_payloads =
        build_artifact_manifest(&module, &kernel, Some(b"same"), Some(b"same")).unwrap();
    assert_ne!(
        equal_payloads.installation(),
        equal_payloads.debug(),
        "section roles must domain-separate identical bytes"
    );

    validate_artifact_manifest(
        &module,
        &kernel,
        Some(b"provider=A"),
        Some(b"source-map=A"),
        first,
    )
    .unwrap();
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &certificate,
            Some(b"provider=A"),
            Some(b"source-map=A"),
            first,
        ),
        Err(ArtifactManifestError::ManifestMismatch)
    );
}

fn representative_bundle() -> ProofBundle {
    let equality = Proposition::Equal(
        ScalarTerm::value(value_id(2), ScalarType::Integer(i32_type())),
        ScalarTerm::value(value_id(1), ScalarType::Integer(i32_type())),
    );
    ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![
            ObligationEvidence {
                obligation: obligation_id(1),
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            },
            ObligationEvidence {
                obligation: obligation_id(2),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(3),
                    proof_system_marker: ProofSystemMarker::CURRENT,
                    proof: ProofNode {
                        conclusion: equality.clone(),
                        rule: ProofRule::EqualityTransitivity {
                            left_equals_middle: Box::new(ProofNode {
                                conclusion: equality.clone(),
                                rule: ProofRule::SemanticAxiom { index: 7 },
                            }),
                            middle_equals_right: Box::new(ProofNode {
                                conclusion: equality,
                                rule: ProofRule::Assumption { index: 5 },
                            }),
                        },
                    },
                }),
            },
            ObligationEvidence {
                obligation: obligation_id(3),
                route: EvidenceRoute::Admitted(AdmissionEvidence {
                    site: AdmissionSiteId::new(4).unwrap(),
                    kind: AdmissionKind::ProviderFact,
                    authority_identity: evidence_id(5),
                    evidence_identity: evidence_id(6),
                    profile_decision: ProfileDecisionId::new(7).unwrap(),
                }),
            },
        ],
    }
}

fn semantic_module() -> TerminalModule {
    let integer = i32_type();
    let scalar_type = ScalarType::Integer(integer);
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type,
            }),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(1),
                    result: psi_terminal::OperationResult::Scalar(ValueDeclaration {
                        id: value_id(1),
                        scalar_type,
                    }),
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: edge_id(1),
                    value: value_id(1),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation: obligation_id(1),
                    proposition: goal,
                }],
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn kernel_bundle() -> ProofBundle {
    ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
        }],
    }
}

fn certificate_bundle() -> ProofBundle {
    let integer = i32_type();
    let literal = ScalarTerm::integer(integer, IntegerValue::Signed(7)).unwrap();
    let goal = Proposition::Equal(literal.clone(), literal);
    ProofBundle {
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(9),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: goal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    }
}

fn i32_type() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("test identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(place_id, PlaceId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_case_id, StructuralCaseId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(evidence_id, EvidenceIdentity);
