//! Unsupported payloadless-case materialization fences at its exact producer.

use omega_psi_to_abstract_operations::{
    ArtifactLoweringError, LoweringError, lower_artifact_sections,
};
use psi_core::StructuralPlaceKind;
use psi_proof_admission::AdmissionProfile;
use psi_terminal::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralCaseDeclaration,
    StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_verifier::ProofBundle;

use super::support::{block_id, contract_id, edge_id, machine_id, place_id, structural_type_id};

#[test]
fn omega_fences_verified_payloadless_case_materialization() {
    let operation = psi_core::OperationId::new(91).unwrap();
    let operation_place = place_id(91);
    let result_place = place_id(92);
    let structural_type = structural_type_id(91);
    let result_case = psi_core::StructuralCaseId::new(91).unwrap();
    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(91),
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::Outcome".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    StructuralCaseDeclaration {
                        id: result_case,
                        identity: "Success".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: psi_core::StructuralCaseId::new(92).unwrap(),
                        identity: "Failure".into(),
                        fields: Vec::new(),
                    },
                ],
            },
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
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine_id(91),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Structural(StructuralResultDeclaration {
                place: result_place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: operation_place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation,
                        structural_type,
                    },
                },
                StructuralPlaceDeclaration {
                    id: result_place,
                    kind: StructuralPlaceKind::Result,
                },
            ],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block_id(91),
            blocks: vec![Block {
                id: block_id(91),
                parameters: Vec::new(),
                operations: vec![Operation {
                    id: operation,
                    result: OperationResult::Structural(StructuralOperationResult {
                        place: operation_place,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    }),
                    kind: OperationKind::EstablishPayloadlessCase { result_case },
                }],
                terminator: Terminator::ReturnStructural {
                    edge: edge_id(91),
                    source: operation_place,
                    returned_claims: Vec::new(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: contract_id(91),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = encode_module(&module).expect("the payloadless case module verifies");
    let proof = encode_proof_bundle(&ProofBundle::default()).expect("empty proof encodes");
    let result = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default());
    assert!(
        matches!(
            result,
            Err(ArtifactLoweringError::Lowering(
                LoweringError::UnsupportedPayloadlessCase(rejected_operation)
            )) if rejected_operation == operation
        ),
        "unexpected result: {result:?}"
    );

    let mut called = module;
    let mut callee = called.machines.remove(0);
    callee.id = machine_id(92);
    callee.entry = block_id(92);
    callee.blocks[0].id = block_id(92);
    callee.contract.id = contract_id(92);
    let call = psi_core::OperationId::new(93).unwrap();
    let caller = TerminalMachine {
        id: machine_id(91),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: place_id(94),
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }),
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place_id(93),
                kind: StructuralPlaceKind::OperationResult {
                    producer: call,
                    structural_type,
                },
            },
            StructuralPlaceDeclaration {
                id: place_id(94),
                kind: StructuralPlaceKind::Result,
            },
        ],
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(91),
        blocks: vec![Block {
            id: block_id(91),
            parameters: Vec::new(),
            operations: vec![Operation {
                id: call,
                result: OperationResult::Structural(StructuralOperationResult {
                    place: place_id(93),
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::CallStructural {
                    callee: machine_id(92),
                    structural_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    returned_claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                    selected_evidence: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnStructural {
                edge: edge_id(93),
                source: place_id(93),
                returned_claims: Vec::new(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract_id(91),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    called.machines = vec![caller, callee];
    let semantic = encode_module(&called).expect("payloadless caller verifies");
    let result = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default());
    assert!(
        matches!(
            result,
            Err(ArtifactLoweringError::Lowering(
                LoweringError::UnsupportedPayloadlessCase(rejected_operation)
            )) if rejected_operation == call
        ),
        "the call itself owns the target-lowering fence: {result:?}"
    );
}
