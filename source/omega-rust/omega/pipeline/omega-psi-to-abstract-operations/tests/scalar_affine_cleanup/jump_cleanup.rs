//! Verified jump affine cleanup is retained without synthesizing an operation.

use omega_abstract_operations::AbstractOperation;
use omega_psi_to_abstract_operations::{
    build_verified_psi_optimization_unit, lower_artifact_sections,
    lower_artifact_sections_for_optimization,
};
use psi_core::{ScalarType, StructuralPlaceKind};
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    ValueDeclaration, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

use super::support::{
    block_id, contract_id, edge_id, machine_id, place_id, structural_type_id, value_id,
};

#[test]
fn omega_consumes_verified_jump_affine_cleanup_without_emitting_an_operation() {
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
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
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
            parameters: vec![ValueDeclaration {
                id: value_id(1),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: vec![StructuralParameterDeclaration {
                access: StructuralAccess::Owned,
                place,
                position: 0,
                is_self: false,
                structural_type: structural_type_id(1),
                multiplicity: StructuralMultiplicity::Affine,
                qualifications: Vec::new(),
            }],
            ranked_scc: None,
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
            blocks: vec![
                Block {
                    id: block_id(1),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge: edge_id(1),
                        target: block_id(2),
                        arguments: vec![value_id(1)],
                        trivial_affine_discards: vec![place],
                    },
                },
                Block {
                    id: block_id(2),
                    parameters: vec![ValueDeclaration {
                        id: value_id(3),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        edge: edge_id(2),
                        value: value_id(3),
                        cleanup_actions: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                id: contract_id(1),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantics = encode_module(&module).expect("exact jump affine cleanup should encode");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof should encode");

    let plan = lower_artifact_sections(&semantics, &proof, &AdmissionProfile::default())
        .expect("verified jump affine cleanup should lower through Omega");
    let [function] = plan.functions.as_slice() else {
        panic!("fixture has one terminal function")
    };
    let [
        AbstractOperation::Jump {
            psi_edge: jump_edge,
            target,
            bindings,
            trivial_affine_discards,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value,
            scalar_type,
            ..
        },
    ] = function.operations.as_slice()
    else {
        panic!("no-code cleanup must not add an abstract operation")
    };
    assert_eq!(*jump_edge, edge_id(1));
    assert_eq!(*target, block_id(2));
    assert_eq!(trivial_affine_discards, &[place]);
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].parameter, value_id(3));
    assert_eq!(bindings[0].argument, value_id(1));
    assert_eq!(*psi_edge, edge_id(2));
    assert_eq!(*result, value_id(2));
    assert_eq!(*value, value_id(3));
    assert_eq!(*scalar_type, ScalarType::Boolean);

    let optimizer_input =
        lower_artifact_sections_for_optimization(&semantics, &proof, &AdmissionProfile::default())
            .expect("verified jump cleanup retains optimizer context");
    let verified = build_verified_psi_optimization_unit(
        optimizer_input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("verified jump cleanup enters optimizer admission");
    let edge = &verified.unit().functions[0].blocks[0].nodes[0].successors[0];
    assert_eq!(edge.psi_edge, edge_id(1));
    assert_eq!(edge.trivial_affine_discards, [place]);
}
