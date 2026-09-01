use omega_abstract_operations::AbstractOperation;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use psi_core::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    StructuralTypeId,
};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, BoundaryMachineDeclaration, ByteSequenceCarrier, MachineContract, Operation,
    OperationKind, OperationResult, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

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
            }],
            result: None,
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
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
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
                kind: psi_core::StructuralPlaceKind::ByteSequenceLiteral {
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
