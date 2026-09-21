use super::{certificate_bundle, kernel_bundle, representative_bundle, semantic_module};
use crate::artifact::{evidence_id, obligation_id};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, PlaceId, Proposition,
    PropositionContext, ScalarTerm, StructuralPlaceKind,
};
use terminal_codec::{
    ArtifactManifestError, ProofCodecError, build_artifact_manifest,
    build_identity_optimization_execution_record, decode_proof_bundle, encode_proof_bundle,
    terminal_psi_identity, validate_artifact_manifest,
};
use terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn proof_format_canonically_encodes_content_certificates() {
    let root = PlaceId::new(80).expect("place");
    let term = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(81).expect("domain"),
            projection_report_fingerprint: 0x8283,
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    proof_admission::check_certificate(&context, &goal, &[], &[], &proof)
        .expect("reflexive content certificate");

    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(
        &semantic_vocabulary::PropositionContext::default(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(
        &semantic_vocabulary::PropositionContext::default(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(
        &semantic_vocabulary::PropositionContext::default(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(
        &semantic_vocabulary::PropositionContext::default(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}

#[test]
fn proof_format_canonically_encodes_sum_case_content_certificates() {
    let root = PlaceId::new(90).expect("place");
    let term = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(91).expect("domain"),
            projection_report_fingerprint: 0x9293,
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(
        &semantic_vocabulary::PropositionContext::default(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
            rule: ProofRule::IntegerOrderSubstitution {
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    let kernel_optimization =
        build_identity_optimization_execution_record(&module, &kernel).unwrap();
    let certificate_optimization =
        build_identity_optimization_execution_record(&module, &certificate).unwrap();
    let first = build_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"provider=A"),
        Some(b"source-map=A"),
    )
    .unwrap();
    let replacement = build_artifact_manifest(
        &module,
        &certificate,
        &certificate_optimization,
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

    let reinstalled = build_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"provider=B"),
        Some(b"source-map=A"),
    )
    .unwrap();
    assert_eq!(first.semantic(), reinstalled.semantic());
    assert_eq!(first.obligations(), reinstalled.obligations());
    assert_eq!(first.proof(), reinstalled.proof());
    assert_ne!(first.installation(), reinstalled.installation());
    assert_ne!(first.identity(), reinstalled.identity());

    let stripped_debug = build_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"provider=A"),
        None,
    )
    .unwrap();
    assert_eq!(first.semantic(), stripped_debug.semantic());
    assert_eq!(first.proof(), stripped_debug.proof());
    assert_eq!(first.installation(), stripped_debug.installation());
    assert_ne!(first.debug(), stripped_debug.debug());
    assert_ne!(first.identity(), stripped_debug.identity());

    let empty_debug = build_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"provider=A"),
        Some(b""),
    )
    .unwrap();
    assert_ne!(stripped_debug.debug(), empty_debug.debug());
    assert_ne!(stripped_debug.identity(), empty_debug.identity());

    let equal_payloads = build_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"same"),
        Some(b"same"),
    )
    .unwrap();
    assert_ne!(
        equal_payloads.installation(),
        equal_payloads.debug(),
        "section roles must domain-separate identical bytes"
    );

    validate_artifact_manifest(
        &module,
        &kernel,
        &kernel_optimization,
        Some(b"provider=A"),
        Some(b"source-map=A"),
        first,
    )
    .unwrap();
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &certificate,
            &certificate_optimization,
            Some(b"provider=A"),
            Some(b"source-map=A"),
            first,
        ),
        Err(ArtifactManifestError::ManifestMismatch)
    );
}

fn bundle_with_proof(proof: ProofNode) -> ProofBundle {
    ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        }],
    }
}

#[test]
fn proof_proposition_nesting_has_a_total_bound() {
    let mut proposition = Proposition::Truth;
    for _ in 0..257 {
        proposition = Proposition::Implication {
            premise: Box::new(proposition),
            conclusion: Box::new(Proposition::Truth),
        };
    }
    let bundle = bundle_with_proof(ProofNode {
        conclusion: proposition,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    });

    assert_eq!(
        encode_proof_bundle(&bundle),
        Err(ProofCodecError::PropositionNestingTooDeep)
    );
}

#[test]
fn proof_content_term_nesting_has_a_total_bound() {
    let leaf = ContentTerm::Projection {
        projection: ContentProjectionIdentity {
            domain: ContentDomainId::new(70).expect("domain"),
            projection_report_fingerprint: 0x7071,
        },
        subject: ContentStructuralPlace {
            version: ContentPlaceVersion::Entry,
            root: PlaceId::new(71).expect("place"),
            segments: Vec::new(),
        },
    };
    // Direct `Separate` construction keeps nested separations reachable for the
    // guard, unlike the canonicalizing `ContentTerm::separate` constructor.
    let mut term = leaf.clone();
    for _ in 0..257 {
        term = ContentTerm::Separate(vec![term, leaf.clone()]);
    }
    let bundle = bundle_with_proof(ProofNode {
        conclusion: Proposition::ContentConservation(ContentConservation::new(
            ContentAlgebra {
                kind: ContentAlgebraKind::CountedQuantity,
                parameter: "Byte".to_owned(),
            },
            leaf,
            term,
        )),
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    });

    assert_eq!(
        encode_proof_bundle(&bundle),
        Err(ProofCodecError::ContentTermNestingTooDeep)
    );
}

#[test]
fn proof_integer_math_term_nesting_has_a_total_bound() {
    let literal = || IntegerMathTerm::literal(IntegerValue::Unsigned(1));
    let mut term = literal();
    for _ in 0..257 {
        term = IntegerMathTerm::Add(Box::new(term), Box::new(literal()));
    }
    let bundle = bundle_with_proof(ProofNode {
        conclusion: Proposition::IntegerMathEqual(term, literal()),
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    });

    assert_eq!(
        encode_proof_bundle(&bundle),
        Err(ProofCodecError::ScalarTermNestingTooDeep)
    );
}
