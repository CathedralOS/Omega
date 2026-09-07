use abstract_operations::AbstractOperation;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    StructuralTypeId,
};
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, ByteSequenceCarrier, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};
use terminal_psi_to_abstract_operations::lower_artifact_sections;
use terminal_verifier::ProofBundle;

#[test]
fn preserves_exact_non_utf8_literal_and_structural_source() {
    let literal_bytes = vec![0, 0x7f, 0x80, 0xff];
    let module = byte_sequence_module(literal_bytes.clone());
    let semantic = encode_module(&module).expect("byte-sequence semantics encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified byte-sequence artifact lowers");

    let [
        AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            bytes,
        },
        AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        },
        AbstractOperation::ReturnUnit { .. },
    ] = plan.functions[0].operations.as_slice()
    else {
        panic!("literal, boundary call, and Unit return must remain ordered")
    };
    assert_eq!(*psi_operation, operation_id(1));
    assert_eq!(place.id, place_id(1));
    assert_eq!(structural_type, &module.structural_types[0]);
    assert_eq!(bytes, &literal_bytes);
    assert_eq!(
        structural_arguments,
        &[StructuralArgument {
            place: place_id(1),
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
        }]
    );
}

#[test]
fn byte_sequence_length_has_an_explicit_native_realization_fence() {
    let mut module = byte_sequence_module(vec![0, 0xff]);
    module.machines[0].blocks[0].operations.insert(
        1,
        Operation {
            id: operation_id(3),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                id: semantic_vocabulary::ValueId::new(1).unwrap(),
                scalar_type: semantic_vocabulary::ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .unwrap(),
                ),
            }),
            kind: OperationKind::ByteSequenceLength {
                source: place_id(1),
            },
        },
    );
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
    assert!(
        matches!(lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()), Err(terminal_psi_to_abstract_operations::ArtifactLoweringError::Lowering(terminal_psi_to_abstract_operations::LoweringError::UnsupportedByteSequenceLength(operation))) if operation == operation_id(3))
    );
}

#[test]
fn byte_sequence_read_has_an_explicit_native_realization_fence_after_verification() {
    byte_operation_fence(false);
}

#[test]
fn byte_sequence_subslice_has_an_explicit_native_realization_fence_after_verification() {
    byte_operation_fence(true);
}

fn byte_operation_fence(subslice: bool) {
    use proof_admission::{
        CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
    };
    use semantic_vocabulary::{
        EvidenceIdentity, IntegerSign, IntegerType, ObligationId, Proposition, ScalarTerm,
        ScalarType, ValueId,
    };
    use terminal_psi::{SuccessorEdge, ValueDeclaration};
    let mut module = byte_sequence_module(Vec::new());
    module.boundary_machines.clear();
    let machine = &mut module.machines[0];
    machine.entry = block_id(2);
    let count_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |ordinal| ValueId::new(ordinal).unwrap();
    machine.parameters = vec![ValueDeclaration {
        id: value(1),
        scalar_type: count_type,
    }];
    machine.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::Parameter {
        position: 0,
        is_self: false,
    };
    machine.structural_parameters = vec![StructuralParameterDeclaration {
        place: place_id(1),
        position: 0,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let successor = |edge, block| SuccessorEdge {
        structural_arguments: Vec::new(),
        edge: edge_id(edge),
        target: block_id(block),
        arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    // Storage order need not be execution order. Visit the read block first
    // during native lowering to isolate its fence from the separate length fence;
    // verification still requires the entry length and true edge to dominate it.
    machine.blocks = vec![
        Block {
            structural_parameters: Vec::new(),
            id: block_id(1),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation_id(3),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: value(4),
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    ),
                }),
                kind: OperationKind::ByteSequenceRead {
                    source: place_id(1),
                    index: value(1),
                    length: value(2),
                    obligation: ObligationId::new(1).unwrap(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: edge_id(3),
                trivial_affine_discards: Vec::new(),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(2),
            parameters: Vec::new(),
            operations: vec![
                Operation {
                    id: operation_id(1),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value(2),
                        scalar_type: count_type,
                    }),
                    kind: OperationKind::ByteSequenceLength {
                        source: place_id(1),
                    },
                },
                Operation {
                    id: operation_id(2),
                    result: OperationResult::Scalar(ValueDeclaration {
                        id: value(3),
                        scalar_type: ScalarType::Boolean,
                    }),
                    kind: OperationKind::IntegerLessThan {
                        left: value(1),
                        right: value(2),
                    },
                },
            ],
            terminator: Terminator::Conditional {
                condition: value(3),
                when_true: successor(1, 1),
                when_false: successor(2, 3),
            },
        },
        Block {
            structural_parameters: Vec::new(),
            id: block_id(3),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(4),
                trivial_affine_discards: Vec::new(),
            },
        },
    ];
    if subslice {
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place_id(3),
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation_id(3),
                structural_type: StructuralTypeId::new(1).unwrap(),
            },
        });
        machine.blocks[0].operations[0].result =
            OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place: place_id(3),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            });
        machine.blocks[0].operations[0].kind = OperationKind::ByteSequenceSubslice {
            source: place_id(1),
            start: value(1),
            end: value(2),
            length: value(2),
            obligation: ObligationId::new(1).unwrap(),
        };
    }
    let questions = terminal_verifier::reconstruct_terminal_obligations(&module).unwrap();
    let [site] = questions.obligations() else {
        panic!("one read question")
    };
    let index = site
        .semantic_axioms
        .iter()
        .position(|fact| {
            fact == &Proposition::LessThan(
                ScalarTerm::value(value(1), count_type),
                ScalarTerm::value(value(2), count_type),
            )
        })
        .unwrap();
    let rule = if subslice {
        let Proposition::Conjunction(children) = &site.obligation.proposition else {
            panic!("range")
        };
        ProofRule::ConjunctionIntroduction(vec![
            ProofNode {
                conclusion: children[0].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: site.semantic_axioms[index].clone(),
                        rule: ProofRule::SemanticAxiom { index },
                    }),
                },
            },
            ProofNode {
                conclusion: children[1].clone(),
                rule: ProofRule::IntegerOrderWeakening {
                    relation: Box::new(ProofNode {
                        conclusion: Proposition::Equal(
                            ScalarTerm::value(value(2), count_type),
                            ScalarTerm::value(value(2), count_type),
                        ),
                        rule: ProofRule::Primitive(
                            proof_admission::PrimitiveJudgment::ReflexiveEquality,
                        ),
                    }),
                },
            },
        ])
    } else {
        ProofRule::SemanticAxiom { index }
    };
    let bundle = ProofBundle {
        evidence: vec![terminal_verifier::ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(1).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof: ProofNode {
                    conclusion: site.obligation.proposition.clone(),
                    rule,
                },
            }),
        }],
        ..ProofBundle::default()
    };
    terminal_verifier::verify_module(&module, &bundle, &AdmissionProfile::default()).unwrap();
    let semantic = encode_module(&module).unwrap();
    let proof = encode_proof_bundle(&bundle).unwrap();
    let error =
        lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap_err();
    match error {
        terminal_psi_to_abstract_operations::ArtifactLoweringError::Lowering(
            terminal_psi_to_abstract_operations::LoweringError::UnsupportedByteSequenceRead(
                operation,
            ),
        ) if !subslice => assert_eq!(operation, operation_id(3)),
        terminal_psi_to_abstract_operations::ArtifactLoweringError::Lowering(
            terminal_psi_to_abstract_operations::LoweringError::UnsupportedByteSequenceSubslice(
                operation,
            ),
        ) if subslice => assert_eq!(operation, operation_id(3)),
        other => panic!("wrong native fence: {other:?}"),
    }
}

fn byte_sequence_module(bytes: Vec<u8>) -> TerminalModule {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let literal = place_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::BorrowedBytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: "test::write_line".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: place_id(2),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![StructuralPlaceDeclaration {
                id: literal,
                kind: semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal: 0,
                    structural_type,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                structural_parameters: Vec::new(),
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: operation_id(1),
                        result: OperationResult::Unit,
                        kind: OperationKind::EstablishByteSequenceLiteral {
                            destination: literal,
                            bytes,
                        },
                    },
                    Operation {
                        id: operation_id(2),
                        result: OperationResult::Unit,
                        kind: OperationKind::BoundaryCall {
                            boundary: boundary_id(1),
                            arguments: Vec::new(),
                            structural_arguments: vec![StructuralArgument {
                                place: literal,
                                access: StructuralAccess::SharedBorrow,
                                path: Vec::new(),
                            }],
                            completion_receipts: Vec::new(),
                        },
                    },
                ],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
fn boundary_id(value: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(value).unwrap()
}
fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}
fn operation_id(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}
fn edge_id(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}
fn place_id(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
