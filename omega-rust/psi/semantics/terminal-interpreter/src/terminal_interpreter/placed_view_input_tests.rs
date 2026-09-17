//! Placed-view input custody at the interpretation boundary.
//!
//! `placed_view_inputs` declares direct entry inputs whose opaque placed-view
//! meaning is bound to one exact source-derived placement interpretation. That
//! roster is semantic custody, not storage: no scalar, structural, or
//! byte-sequence input can supply the referent it names. Interpretation has no
//! establishment route that lends one, so a module whose roster is nonempty
//! must reject at start rather than execute the entry machine with a declared
//! input silently unbound — the same custody gate the plan, optimizer, and
//! native-realization admissions already apply.

use super::{TerminalExecution, TerminalInterpretError};
use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId};
use terminal_psi::{
    Block, MachineContract, StructuralAccess, TerminalMachine, TerminalMachineResult,
    TerminalModule, TerminalPlacedViewInput, Terminator,
};

fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

fn unit_machine(id: u64) -> TerminalMachine {
    TerminalMachine {
        id: machine_id(id),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block_id(id),
        blocks: vec![Block {
            structural_parameters: Vec::new(),
            id: block_id(id),
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: edge_id(id),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(id).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn module(placed_view_inputs: Vec<TerminalPlacedViewInput>) -> TerminalModule {
    TerminalModule {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        entry: machine_id(1),
        scalar_qualifications: Default::default(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs,
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
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![unit_machine(1)],
    }
}

fn placed_view_row(machine: MachineId) -> TerminalPlacedViewInput {
    let policy_identity = "package::Uart".to_string();
    let schema_identity = "package::Registers".to_string();
    TerminalPlacedViewInput {
        machine,
        position: 0,
        source_machine_identity: "package::inspect".into(),
        source_state_identity: "package::inspect::entry".into(),
        source_parameter_identity: "package::inspect::entry::view0".into(),
        access: StructuralAccess::MutableBorrow,
        binding_is_const: false,
        binding_is_mutable: true,
        view_identity: terminal_psi::canonical_placed_view_identity(
            &policy_identity,
            &schema_identity,
        ),
        policy_identity,
        policy_plan_machine_identity: "package::Uart::plan".into(),
        schema_identity,
        placement_report_fingerprint: 41,
        placement_commitment: [0x5a; 32],
    }
}

#[test]
fn placed_view_roster_rejects_at_interpretation_start() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry));
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], None),
        Err(TerminalInterpretError::PlacedViewInputsRequireCustody)
    ));
}

#[test]
fn empty_placed_view_roster_still_starts() {
    assert!(
        TerminalExecution::start_verified_module(module(Vec::new()), &[], &[], &[], &[], None)
            .is_ok()
    );
}
