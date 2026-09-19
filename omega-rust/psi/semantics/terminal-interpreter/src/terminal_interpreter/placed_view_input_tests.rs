//! Placed-view input custody at the interpretation boundary.
//!
//! `placed_view_inputs` declares direct entry inputs whose opaque placed-view
//! meaning is bound to one exact source-derived placement interpretation. That
//! roster is semantic custody, not storage: no scalar, structural, or
//! byte-sequence input can supply the referent it names. The ordinary
//! establishment route supplies one `TerminalPlacedViewEstablishment` per
//! direct-entry roster row — the provider's loan of the exact qualified
//! backing — and interpretation binds it as a live occurrence for the entry
//! invocation's duration. A row left unsupplied, a supply answering no
//! declared row, and a row on a non-entry machine all reject at start rather
//! than execute the entry machine with a declared input silently unbound.

use super::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalPlacedViewEstablishment, TerminalStructuralValue,
};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, StructuralDomainId, StructuralTypeId,
};
use terminal_fuel::TerminalFuelMeter;
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
        machines: vec![unit_machine(1), unit_machine(2)],
    }
}

const PACKAGE_DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn placed_view_row(machine: MachineId, position: u32) -> TerminalPlacedViewInput {
    let policy_identity = format!("package:{PACKAGE_DIGEST}::Uart");
    let schema_identity = format!("package:{PACKAGE_DIGEST}::Registers");
    TerminalPlacedViewInput {
        machine,
        position,
        source_machine_identity: format!("package:{PACKAGE_DIGEST}::inspect"),
        source_state_identity: format!("package:{PACKAGE_DIGEST}::inspect::entry"),
        source_parameter_identity: format!(
            "package:{PACKAGE_DIGEST}::inspect::entry::view{position}"
        ),
        access: StructuralAccess::MutableBorrow,
        binding_is_const: false,
        binding_is_mutable: true,
        view_identity: terminal_psi::canonical_placed_view_identity(
            &policy_identity,
            &schema_identity,
        ),
        policy_identity,
        policy_plan_machine_identity: format!("package:{PACKAGE_DIGEST}::Uart::plan"),
        schema_identity,
        placement_report_fingerprint: 41 + u64::from(position),
        placement_commitment: [0x5a; 32],
    }
}

fn referent(opaque_identity: u64) -> TerminalStructuralValue {
    TerminalStructuralValue {
        opaque_identity,
        structural_type: StructuralTypeId::new(1).unwrap(),
        qualifications: vec![StructuralDomainId::new(7).unwrap()],
        path: Vec::new(),
    }
}

fn establishment(
    input: TerminalPlacedViewInput,
    opaque_identity: u64,
) -> TerminalPlacedViewEstablishment {
    TerminalPlacedViewEstablishment {
        input,
        referent: referent(opaque_identity),
    }
}

#[test]
fn placed_view_roster_rejects_at_interpretation_start() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &[], None),
        Err(TerminalInterpretError::PlacedViewInputsRequireCustody)
    ));
}

#[test]
fn empty_placed_view_roster_still_starts() {
    assert!(
        TerminalExecution::start_verified_module(module(Vec::new()), &[], &[], &[], &[], &[], None)
            .is_ok()
    );
}

#[test]
fn established_placed_view_binds_and_retires_at_completion() {
    let mut placed = module(Vec::new());
    let row = placed_view_row(placed.entry, 0);
    placed.placed_view_inputs.push(row);
    let establishments = [establishment(
        placed.placed_view_inputs[0].clone(),
        0xC0FFEE,
    )];
    let mut execution =
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None)
            .expect("an exact establishment supplies the declared input's custody");
    assert_eq!(execution.placed_view_occurrences.len(), 1);
    let mut meter = TerminalFuelMeter::unbounded();
    let status = execution
        .resume(&mut meter, &mut AcceptTerminalEffects)
        .expect("the declared input is bound for the invocation");
    assert!(matches!(
        status,
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert!(
        execution.placed_view_occurrences.is_empty(),
        "completing the entry invocation retires the lent occurrences"
    );
}

#[test]
fn stale_or_substituted_establishment_rejects() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    // A drifted supply — a stale placement commitment — answers no declared
    // row, so it rejects as unexpected rather than binding stale custody.
    let mut drifted = placed_view_row(placed.entry, 0);
    drifted.placement_commitment = [0xee; 32];
    let establishments = [establishment(drifted, 0xC0FFEE)];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::PlacedViewInputEstablishmentUnexpected { .. })
    ));
}

#[test]
fn establishment_for_undeclared_input_rejects() {
    let placed = module(Vec::new());
    let establishments = [establishment(placed_view_row(machine_id(1), 0), 0xC0FFEE)];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::PlacedViewInputEstablishmentUnexpected { .. })
    ));
}

#[test]
fn duplicate_establishment_rejects() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    let row = placed.placed_view_inputs[0].clone();
    let establishments = [
        establishment(row.clone(), 0xC0FFEE),
        establishment(row, 0xBEEF),
    ];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::PlacedViewInputEstablishmentDuplicate { .. })
    ));
}

#[test]
fn non_entry_roster_row_still_requires_custody() {
    let mut placed = module(Vec::new());
    // A row on a non-entry machine declares a call-bound input; this input
    // boundary cannot route its custody, so it keeps failing closed.
    placed
        .placed_view_inputs
        .push(placed_view_row(machine_id(2), 0));
    let establishments = [establishment(
        placed.placed_view_inputs[0].clone(),
        0xC0FFEE,
    )];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::PlacedViewInputsRequireCustody)
    ));
}

#[test]
fn non_canonical_referent_qualifications_reject() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    let mut supply = establishment(placed.placed_view_inputs[0].clone(), 0xC0FFEE);
    supply.referent.qualifications = vec![
        StructuralDomainId::new(9).unwrap(),
        StructuralDomainId::new(7).unwrap(),
    ];
    let establishments = [supply];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::StructuralQualificationsNonCanonical)
    ));
}

#[test]
fn exclusive_establishment_referent_aliasing_rejects() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 1));
    let establishments = [
        establishment(placed.placed_view_inputs[0].clone(), 0xC0FFEE),
        establishment(placed.placed_view_inputs[1].clone(), 0xC0FFEE),
    ];
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &establishments, None),
        Err(TerminalInterpretError::PlacedViewInputEstablishmentAliasing(0xC0FFEE))
    ));
}

#[test]
fn exclusive_establishment_aliasing_structural_argument_rejects() {
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    let arguments = [referent(0xBEEF)];
    let establishments = [establishment(placed.placed_view_inputs[0].clone(), 0xBEEF)];
    assert!(matches!(
        TerminalExecution::start_verified_module(
            placed,
            &[],
            &arguments,
            &[],
            &[],
            &establishments,
            None
        ),
        Err(TerminalInterpretError::PlacedViewInputEstablishmentAliasing(0xBEEF))
    ));
}
