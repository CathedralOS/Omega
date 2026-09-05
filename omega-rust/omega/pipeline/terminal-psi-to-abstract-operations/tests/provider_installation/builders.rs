use super::ids::{
    REQUIREMENT, block_id, boundary_id, contract_id, edge_id, machine_id, operation_id, service_id,
    structural_type_id,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, OperationId, ServiceId, StructuralTypeId,
};
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_psi::{
    Block, BoundaryMachineDeclaration, MachineContract, Operation, OperationKind, OperationResult,
    ProviderCandidateConformance, ProviderUnitRefinement, ProviderUnitSignature,
    ServiceDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, VocabularyMarker,
};
use terminal_psi_to_abstract_operations::SelectedProviderAdapter;
use terminal_verifier::ProofBundle;

pub(super) fn selected(_name: &str, provider: &str, machine: &str) -> Vec<SelectedProviderAdapter> {
    vec![SelectedProviderAdapter {
        requirement_identity: REQUIREMENT.into(),
        provider_identity: provider.into(),
        machine_identity: machine.into(),
    }]
}

pub(super) fn artifact(module: &TerminalModule) -> (Vec<u8>, Vec<u8>) {
    (
        encode_module(module).expect("semantic section"),
        encode_proof_bundle(&ProofBundle::default()).expect("proof section"),
    )
}

pub(super) fn provider_module() -> TerminalModule {
    let service = service_id(1);
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: machine_id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: structural_type_id(1),
                identity: "named(name(FirstProvider))".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
            StructuralTypeDeclaration {
                id: structural_type_id(2),
                identity: "named(name(SecondProvider))".into(),
                shape: StructuralTypeShape::Record { fields: Vec::new() },
            },
        ],
        structural_domains: Vec::new(),
        services: vec![ServiceDeclaration {
            id: service,
            identity: "Signal".into(),
            parents: Vec::new(),
        }],
        root_service_reach: terminal_psi::TerminalRootServiceReach {
            concrete: vec![service],
            installation_dependencies: Vec::new(),
        },
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary_id(1),
            identity: REQUIREMENT.into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: vec![
            candidate(
                "FirstProvider",
                "FirstProvider::emit",
                machine_id(2),
                service,
            ),
            candidate(
                "SecondProvider",
                "SecondProvider::emit",
                machine_id(3),
                service,
            ),
        ],
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
        machines: vec![
            machine(
                machine_id(1),
                None,
                block_id(1),
                edge_id(1),
                contract_id(1),
                Operation {
                    id: operation_id(1),
                    result: OperationResult::Unit,
                    kind: OperationKind::BoundaryCall {
                        boundary: boundary_id(1),
                        arguments: Vec::new(),
                        structural_arguments: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                },
                service,
            ),
            machine(
                machine_id(2),
                Some(structural_type_id(1)),
                block_id(2),
                edge_id(2),
                contract_id(2),
                port_write(operation_id(2), service, 65),
                service,
            ),
            machine(
                machine_id(3),
                Some(structural_type_id(2)),
                block_id(3),
                edge_id(3),
                contract_id(3),
                port_write(operation_id(3), service, 66),
                service,
            ),
        ],
    }
}

fn candidate(
    provider_identity: &str,
    candidate_identity: &str,
    candidate: MachineId,
    service: ServiceId,
) -> ProviderCandidateConformance {
    ProviderCandidateConformance {
        boundary: boundary_id(1),
        requirement_identity: REQUIREMENT.into(),
        provider_identity: provider_identity.into(),
        candidate_identity: candidate_identity.into(),
        candidate,
        signature: ProviderUnitSignature {
            parameters: Vec::new(),
        },
        refinement: ProviderUnitRefinement {
            positional_parameters: Vec::new(),
            required_domains: Vec::new(),
            realized_service_ceiling: vec![service],
        },
    }
}

fn machine(
    id: MachineId,
    attachment: Option<StructuralTypeId>,
    block: BlockId,
    edge: EdgeId,
    contract: ContractId,
    operation: Operation,
    service: ServiceId,
) -> TerminalMachine {
    TerminalMachine {
        id,
        attachment,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        published_service_ceiling: vec![service],
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![operation],
            terminator: Terminator::ReturnUnit {
                edge,
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: contract,
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn port_write(id: OperationId, service: ServiceId, value: u8) -> Operation {
    Operation {
        id,
        result: OperationResult::Unit,
        kind: OperationKind::PortWrite {
            service,
            port: 0x80,
            value,
        },
    }
}
