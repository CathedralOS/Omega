use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use psi_core::{
    BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, OperationId, ScalarType, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, BoundaryMachineDeclaration, MachineContract, Operation, OperationKind, OperationResult,
    TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
    VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

#[test]
fn preserves_scalar_boundary_arguments_in_authored_order() {
    let boolean = ValueDeclaration {
        id: value_id(1),
        scalar_type: ScalarType::Boolean,
    };
    let byte_type = ScalarType::Integer(
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8)
            .expect("u8 is a valid integer type"),
    );
    let byte = ValueDeclaration {
        id: value_id(2),
        scalar_type: byte_type,
    };
    let boundary = boundary_id(1);
    let operation = operation_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Console::write_byte(u8,bool)->Unit".into(),
            attachment: None,
            scalar_parameters: vec![byte_type, ScalarType::Boolean],
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
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
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: vec![boolean, byte],
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(1),
            blocks: vec![Block {
                id: block_id(1),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCall {
                        boundary,
                        arguments: vec![byte.id, boolean.id],
                        structural_arguments: Vec::new(),
                        completion_receipts: Vec::new(),
                        requirement_obligations: Vec::new(),
                    },
                }],
                terminator: Terminator::ReturnUnit {
                    edge: edge_id(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
            },
        }],
    };
    let semantic = encode_module(&module).expect("scalar boundary artifact encodes");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof bundle encodes");
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("verified scalar boundary call lowers into Omega");

    assert_eq!(plan.boundary_machines, module.boundary_machines);
    let [
        TerminalAbstractOperation::BoundaryCall {
            psi_operation,
            boundary: lowered_boundary,
            arguments,
            ..
        },
        TerminalAbstractOperation::ReturnUnit { .. },
    ] = plan.functions[0].operations.as_slice()
    else {
        panic!("fixture lowers to a boundary call followed by Unit return")
    };
    assert_eq!(*psi_operation, operation);
    assert_eq!(*lowered_boundary, boundary);
    assert_eq!(arguments, &[byte.id, boolean.id]);
}

fn machine_id(value: u32) -> MachineId {
    MachineId::new(u64::from(value)).unwrap()
}

fn boundary_id(value: u32) -> BoundaryMachineId {
    BoundaryMachineId::new(u64::from(value)).unwrap()
}

fn block_id(value: u32) -> BlockId {
    BlockId::new(u64::from(value)).unwrap()
}

fn operation_id(value: u32) -> OperationId {
    OperationId::new(u64::from(value)).unwrap()
}

fn edge_id(value: u32) -> EdgeId {
    EdgeId::new(u64::from(value)).unwrap()
}

fn contract_id(value: u32) -> ContractId {
    ContractId::new(u64::from(value)).unwrap()
}

fn value_id(value: u32) -> ValueId {
    ValueId::new(u64::from(value)).unwrap()
}
