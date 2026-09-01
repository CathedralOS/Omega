use psi_core::{BlockId, ContractId, EdgeId, MachineId, PsiSemanticId};
use psi_terminal::{
    Block, MachineContract, TerminalMachine, TerminalMachineResult, TerminalModule, Terminator,
    VocabularyMarker,
};

use super::{decode_module, encode_module};
use crate::module_wire::encode_legacy_result_path_raw;

fn id<T: PsiSemanticId>(raw: u64) -> T {
    T::new(raw).expect("test ids are nonzero")
}

fn unit_module() -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: id::<MachineId>(1),
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
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: id::<MachineId>(1),
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
            entry: id::<BlockId>(1),
            blocks: vec![Block {
                id: id::<BlockId>(1),
                parameters: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::ReturnUnit {
                    edge: id::<EdgeId>(1),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: id::<ContractId>(1),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
                crash_routes: Vec::new(),
            },
        }],
    }
}

#[test]
fn v56_v59_reconstructs_absent_result_path_rosters_as_current_empty_rows() {
    let module = unit_module();
    let legacy = encode_legacy_result_path_raw(&module).expect("legacy compatibility bytes");
    assert_eq!(&legacy[8..10], &56_u16.to_le_bytes());
    assert_eq!(&legacy[10..12], &59_u16.to_le_bytes());
    assert_eq!(decode_module(&legacy), Ok(module.clone()));

    let current = encode_module(&module).expect("current result-path bytes");
    assert_eq!(&current[8..10], &60_u16.to_le_bytes());
    assert_eq!(&current[10..12], &63_u16.to_le_bytes());

    let mut crossed_pair = legacy;
    crossed_pair[10..12].copy_from_slice(&63_u16.to_le_bytes());
    assert!(decode_module(&crossed_pair).is_err());
}
