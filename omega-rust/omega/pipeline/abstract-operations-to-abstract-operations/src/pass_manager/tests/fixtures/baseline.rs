//! Empty verified-unit baseline.

use super::super::VerifiedPsiOptimizationUnit;

pub(in crate::pass_manager::tests) fn verified_empty_unit() -> VerifiedPsiOptimizationUnit {
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
    use terminal_psi::{
        Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
        VocabularyMarker,
    };

    let module = TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(401).unwrap(),
        structural_types: Vec::new(),
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
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: MachineId::new(401).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(402).unwrap(),
            blocks: vec![Block {
                id: BlockId::new(402).unwrap(),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(403).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(404).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof =
        terminal_codec::encode_proof_bundle(&terminal_verifier::ProofBundle::default()).unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
