use super::{certificate_bundle, semantic_module};
use crate::artifact::{
    evidence_id, machine_id, obligation_id, place_id, structural_case_id, structural_field_id,
    value_id,
};
use proof_admission::{
    AdmissionProfile, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, Proposition,
    PropositionContext, ScalarTerm, StructuralCaseSubject,
};
use terminal_codec::{
    ProofCodecError, current_rust_operation_semantics_trust_identity, current_terminal_trust_graph,
    decode_proof_bundle, encode_proof_bundle, proof_bundle_fingerprint,
    render_verified_proof_synopsis,
};
use terminal_psi::OperationKind;
use terminal_verifier::{ObligationEvidence, ProofBundle, verify_module};

#[test]
fn proof_format_round_trips_structural_case_membership() {
    let bundle = ProofBundle {
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
    assert!(first.contains("certificate 9 proof-system 5"));
    assert!(first.contains("rule Primitive"));
    assert!(
        !first.contains("ranked-countdown "),
        "ordinary verified synopsis bytes do not acquire ranked-only rows"
    );
    let trust_graph = current_terminal_trust_graph().expect("current trust graph");
    assert!(first.contains(&format!(
        "trust-graph {} entry closure:terminal-pcc-current fully-derived false",
        trust_graph.identity()
    )));
    assert!(first.contains("trust-node implementation:rust-terminal-decoder"));
    assert!(first.contains("trust-node implementation:rust-terminal-verifier"));
    assert!(!first.contains("trust-node reduction:"));
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
            erased_arguments: Vec::new(),
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive Boolean-equality certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));

    let mut stale = bytes;
    stale[31..33].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_proof_bundle(&stale),
        Err(ProofCodecError::UnsupportedProofSystemMarker(1))
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-equality certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-ordering certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-bitwise certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-bitwise-not certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive integer-widen certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive address certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-right-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-left-shift certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-add certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-subtract certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-multiply certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive exact-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive wrapping-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive saturating-divide certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive saturating-remainder certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
        recursive_components: Vec::new(),
        control_cycles: Vec::new(),
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

    proof_admission::check_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
        .expect("reflexive Boolean-negation certificate");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
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
    .expect("closed u8 wrapping addition proves 44");
    let bytes = encode_proof_bundle(&bundle).expect("current proof bytes");
    assert_eq!(&bytes[8..10], &33_u16.to_le_bytes());
    assert_eq!(decode_proof_bundle(&bytes), Ok(bundle.clone()));
}
