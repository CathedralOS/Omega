use psi_core::{
    AdmissionSiteId, BlockId, ContractId, EdgeId, EvidenceIdentity, IntegerSign, IntegerType,
    IntegerValue, MachineId, ObligationId, OperationId, ProfileDecisionId, Proposition, ScalarTerm,
    ScalarType, ValueId,
};
use psi_proof_kernel::{
    AdmissionEvidence, AdmissionKind, AdmissionProfile, CertificateEnvelope, EvidenceRoute,
    PrimitiveJudgment, ProofNode, ProofRule, ProofSystemVersion,
};
use psi_terminal::{
    Block, ContractClause, MachineContract, Operation, OperationKind, SemanticVersion,
    TerminalMachine, TerminalModule, Terminator, ValueDeclaration,
};
use psi_terminal_codec::{
    ArtifactManifestError, ProofCodecError, build_artifact_manifest, decode_proof_bundle,
    encode_proof_bundle, proof_bundle_fingerprint, terminal_psi_identity,
    validate_artifact_manifest,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn proof_bundle_has_stable_canonical_bytes_and_an_independent_identity() {
    let bundle = representative_bundle();
    let bytes = encode_proof_bundle(&bundle).expect("representative proof bundle should encode");

    assert_eq!(&bytes[..8], b"PSIPRF\0\0");
    assert_eq!(&bytes[8..10], 1_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "f4b07ab8cdca7a33b0d2184c77db19d11626bf5db938d19ce13afa38628d06c4"
    );

    let mut unnecessarily_v2 = bytes.clone();
    unnecessarily_v2[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&unnecessarily_v2),
        Err(ProofCodecError::NonCanonicalEncoding)
    );

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
    let mut future = bytes;
    future[8..10].copy_from_slice(&7_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&future),
        Err(ProofCodecError::UnsupportedFormatVersion(7))
    );
}

#[test]
fn proof_format_v2_canonically_encodes_closed_wrapping_arithmetic() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(200)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap();
    let sum = ScalarTerm::wrapping_integer_add(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(44)).unwrap();
    let goal = Proposition::Equal(sum, reduced);
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_kernel::check_certificate(
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
    let bytes = encode_proof_bundle(&bundle).expect("proof v2 bytes");
    assert_eq!(&bytes[8..10], &2_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "efe529c7e2ee78900801f6423704d4f7f6312b35b91657dc0bac25adcc9595cd"
    );

    let mut unnecessarily_v3 = bytes;
    unnecessarily_v3[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&unnecessarily_v3),
        Err(ProofCodecError::NonCanonicalEncoding)
    );
}

#[test]
fn proof_format_v3_canonically_encodes_closed_saturating_arithmetic() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(200)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(100)).unwrap();
    let sum = ScalarTerm::saturating_integer_add(integer, left, right).unwrap();
    let clamped = ScalarTerm::integer(integer, IntegerValue::Unsigned(255)).unwrap();
    let goal = Proposition::Equal(sum, clamped);
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_kernel::check_certificate(
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
    let bytes = encode_proof_bundle(&bundle).expect("proof v3 bytes");
    assert_eq!(&bytes[8..10], &3_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "d9cada7d8d15d785b3dbe60b8845032e58a1ee06f40532a4b15b320de495dbf6"
    );
}

#[test]
fn proof_format_v4_canonically_encodes_closed_wrapping_subtraction() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
    let difference = ScalarTerm::wrapping_integer_subtract(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(251)).unwrap();
    let goal = Proposition::Equal(difference, reduced);
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_kernel::check_certificate(
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
    let bytes = encode_proof_bundle(&bundle).expect("proof v4 bytes");
    assert_eq!(&bytes[8..10], &4_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "de2583689692dc8f2031d71d1f6a2d4256890cc3b5583d8d0fe9b36b36f83ecf"
    );
}

#[test]
fn proof_format_v5_canonically_encodes_closed_saturating_subtraction() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(5)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
    let difference = ScalarTerm::saturating_integer_subtract(integer, left, right).unwrap();
    let clamped = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
    let goal = Proposition::Equal(difference, clamped);
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_kernel::check_certificate(
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
    let bytes = encode_proof_bundle(&bundle).expect("proof v5 bytes");
    assert_eq!(&bytes[8..10], &5_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "4ff038480cc5a7573c2a217b4ad4e76fa0f7ce267f84bdb82ab6876395e8deb3"
    );
}

#[test]
fn proof_format_v6_canonically_encodes_closed_wrapping_multiplication() {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let left = ScalarTerm::integer(integer, IntegerValue::Unsigned(20)).unwrap();
    let right = ScalarTerm::integer(integer, IntegerValue::Unsigned(13)).unwrap();
    let product = ScalarTerm::wrapping_integer_multiply(integer, left, right).unwrap();
    let reduced = ScalarTerm::integer(integer, IntegerValue::Unsigned(4)).unwrap();
    let goal = Proposition::Equal(product, reduced);
    let bundle = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof: ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                },
            }),
        }],
    };

    psi_proof_kernel::check_certificate(
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
    let bytes = encode_proof_bundle(&bundle).expect("proof v6 bytes");
    assert_eq!(&bytes[8..10], &6_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
    assert_eq!(
        proof_bundle_fingerprint(&bundle).unwrap().to_string(),
        "ca94daffef56eebbb5ecb44f90e619f7cc85fa62c4a5af0b413f1b60ddf3426a"
    );
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
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(proof),
            },
        };
    }
    let too_deep = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
                proof,
            }),
        }],
    };
    assert_eq!(
        encode_proof_bundle(&too_deep),
        Err(ProofCodecError::ProofNestingTooDeep)
    );

    let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let literal = || ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).unwrap();
    let mut term = literal();
    for _ in 0..257 {
        term = ScalarTerm::wrapping_integer_add(integer, term, literal()).unwrap();
    }
    let deep_term = ProofBundle {
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(1),
                proof_system_version: ProofSystemVersion::CURRENT,
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
    assert_ne!(first.proof(), replacement.proof());
    assert_ne!(first.identity(), replacement.identity());
    assert_eq!(first.installation(), replacement.installation());
    assert_eq!(first.debug(), replacement.debug());

    let reinstalled =
        build_artifact_manifest(&module, &kernel, Some(b"provider=B"), Some(b"source-map=A"))
            .unwrap();
    assert_eq!(first.semantic(), reinstalled.semantic());
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
        evidence: vec![
            ObligationEvidence {
                obligation: obligation_id(1),
                route: EvidenceRoute::KernelDerived(PrimitiveJudgment::Truth),
            },
            ObligationEvidence {
                obligation: obligation_id(2),
                route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                    identity: evidence_id(3),
                    proof_system_version: ProofSystemVersion::CURRENT,
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
        semantic_version: SemanticVersion::CURRENT,
        entry: machine_id(1),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            parameters: Vec::new(),
            result: ValueDeclaration {
                id: value_id(2),
                scalar_type,
            },
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation_id(1),
                    result: ValueDeclaration {
                        id: value_id(1),
                        scalar_type,
                    },
                    kind: OperationKind::IntegerConstant {
                        value: IntegerValue::Signed(7),
                    },
                }],
                terminator: Terminator::Return {
                    edge: edge_id(1),
                    value: value_id(1),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                requires: vec![goal.clone()],
                ensures: vec![ContractClause {
                    obligation: obligation_id(1),
                    proposition: goal,
                }],
            },
        }],
    }
}

fn kernel_bundle() -> ProofBundle {
    ProofBundle {
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
        evidence: vec![ObligationEvidence {
            obligation: obligation_id(1),
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: evidence_id(9),
                proof_system_version: ProofSystemVersion::CURRENT,
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
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(evidence_id, EvidenceIdentity);
