use omega_terminal_abstract_operations::TerminalAbstractOperation;
use omega_terminal_psi_to_abstract_operations::lower_artifact_sections;
use psi_core::{
    BlockId, ContractId, EdgeId, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use psi_proof_kernel::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

#[test]
fn omega_consumes_verified_scalar_affine_cleanup_without_emitting_an_operation() {
    let place = place_id(1);
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type_id(1),
            identity: "test::AffineToken".into(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: Vec::new(),
        services: Vec::new(),
        boundary_machines: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(1),
            attachment: None,
            parameters: vec![ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            result: TerminalMachineResult::Scalar(ValueDeclaration {
                id: value_id(2),
                scalar_type: ScalarType::Boolean,
            }),
            structural_places: vec![StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
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
                operations: Vec::new(),
                terminator: Terminator::Return {
                    edge: edge_id(1),
                    value: value_id(1),
                    trivial_affine_discards: vec![place],
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
    let semantics = encode_module(&module).expect("exact scalar affine cleanup should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified scalar affine cleanup should lower through Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    let [
        TerminalAbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
        },
    ] = function.operations.as_slice()
    else {
        panic!("no-code cleanup must not add an abstract operation")
    };
    assert_eq!(*psi_edge, edge_id(1));
    assert_eq!(*result, value_id(2));
    assert_eq!(*value, value_id(1));
    assert_eq!(*scalar_type, ScalarType::Boolean);
}

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}
