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

#[path = "byte_sequence_literal/byte_sequence_operations.rs"]
mod byte_sequence_operations;

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
fn byte_sequence_length_retains_exact_source_result_type_and_rejects_drift() {
    let mut module = byte_sequence_module(vec![0, 0xff]);
    module.boundary_machines.clear();
    module.machines[0].blocks[0].operations.remove(1);
    module.machines[0].blocks[0].operations.insert(
        1,
        Operation {
            id: operation_id(3),
            result: OperationResult::Scalar(terminal_psi::ValueDeclaration {
                qualifications: Default::default(),
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
    let profile = AdmissionProfile::default();
    let plan = lower_artifact_sections(&semantic, &proof, &profile).unwrap();
    let AbstractOperation::ByteSequenceLength {
        psi_operation,
        result,
        source,
    } = &plan.functions[0].operations[1]
    else {
        panic!("length remains an observation")
    };
    assert_eq!(*psi_operation, operation_id(3));
    assert_eq!(*source, place_id(1));
    assert_eq!(result.value, semantic_vocabulary::ValueId::new(1).unwrap());
    assert_eq!(
        result.scalar_type,
        module.machines[0].blocks[0].operations[1]
            .result
            .scalar()
            .unwrap()
            .scalar_type
    );
    terminal_psi_to_abstract_operations::admit_provider_installation(
        &plan,
        &semantic,
        &proof,
        &profile,
        &[],
    )
    .unwrap();

    let optimization = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&optimization).unwrap();

    for mutation in 0..5 {
        let mut drifted = plan.clone();
        let operation = &mut drifted.functions[0].operations[1];
        let AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            source,
        } = operation
        else {
            panic!("length observation")
        };
        match mutation {
            0 => *psi_operation = operation_id(9),
            1 => *source = place_id(9),
            2 => result.value = semantic_vocabulary::ValueId::new(9).unwrap(),
            3 => result.scalar_type = semantic_vocabulary::ScalarType::Boolean,
            4 => {
                *operation = AbstractOperation::BooleanConstant {
                    psi_operation: *psi_operation,
                    result: result.value,
                    value: false,
                }
            }
            _ => panic!("bounded mutations"),
        }
        assert!(matches!(
            terminal_psi_to_abstract_operations::admit_provider_installation(
                &drifted,
                &semantic,
                &proof,
                &profile,
                &[],
            ),
            Err(terminal_psi_to_abstract_operations::ProviderInstallationError::PlanReplayMismatch)
        ));
        let changed = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &drifted,
            semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert_ne!(optimization.identity, changed.identity);
        if mutation == 1 || mutation == 3 {
            assert!(optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err());
        }
    }

    let mut unavailable = optimization.clone();
    unavailable.functions[0].blocks[0].nodes.swap(0, 1);
    unavailable.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&unavailable);
    assert!(optimization_unit_semantics::validate_psi_optimization_unit(&unavailable).is_err());
}

#[test]
fn byte_sequence_read_retains_exact_verified_operands_and_proof() {
    byte_sequence_operations::byte_operation_fence(false);
}

#[test]
fn byte_sequence_subslice_retains_exact_verified_operands_and_proof() {
    byte_sequence_operations::byte_operation_fence(true);
}

fn byte_sequence_module(bytes: Vec<u8>) -> TerminalModule {
    let structural_type = StructuralTypeId::new(1).unwrap();
    let literal = place_id(1);
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_range_invariants: Vec::new(),
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
            crash_routes: Vec::new(),
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
