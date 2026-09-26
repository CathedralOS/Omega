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

use crate::effects::AcceptTerminalEffects;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::results::{TerminalExecutionResult, TerminalExecutionStatus};
use crate::structural_inputs::placed_views::TerminalPlacedViewEstablishment;
use crate::values::TerminalStructuralValue;
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
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
        structural_types: vec![terminal_psi::StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: "Backing".to_owned(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        }],
        structural_domains: vec![terminal_psi::StructuralDomainDeclaration {
            establishment_routes: Vec::new(),
            id: StructuralDomainId::new(7).unwrap(),
            semantic_domain: semantic_vocabulary::DomainSemanticId::new(7).unwrap(),
            identity: "Backing::Granted".to_owned(),
            carrier: StructuralTypeId::new(1).unwrap(),
            content_projection: None,
        }],
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

#[test]
fn substituted_referent_axes_reject_before_binding() {
    use terminal_semantics::PlacedViewReferentError;
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    let original = establishment(placed.placed_view_inputs[0].clone(), 0xC0FFEE);
    let missing_type = StructuralTypeId::new(99).unwrap();
    let missing_domain = StructuralDomainId::new(99).unwrap();
    let mut backing = original.clone();
    backing.referent.structural_type = missing_type;
    let mut path = original.clone();
    path.referent
        .path
        .push(terminal_psi::StructuralPathSegment::FixedIndex(0));
    let mut qualification = original.clone();
    qualification.referent.qualifications = vec![missing_domain];
    for (supply, expected) in [
        (
            backing,
            PlacedViewReferentError::BackingUndeclared(missing_type),
        ),
        (path, PlacedViewReferentError::RangeUnresolved),
        (
            qualification,
            PlacedViewReferentError::QualificationUndeclared(missing_domain),
        ),
    ] {
        assert!(matches!(
            TerminalExecution::start_verified_module(
                placed.clone(), &[], &[], &[], &[], &[supply], None
            ),
            Err(TerminalInterpretError::PlacedViewReferent(error)) if error == expected
        ));
    }
    // The domain exists, but it qualifies a different declared root carrier.
    placed
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: StructuralTypeId::new(2).unwrap(),
            identity: "OtherBacking".to_owned(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        });
    placed.structural_domains[0].carrier = StructuralTypeId::new(2).unwrap();
    assert!(matches!(
        TerminalExecution::start_verified_module(
            placed, &[], &[], &[], &[], &[original], None
        ),
        Err(TerminalInterpretError::PlacedViewReferent(
            PlacedViewReferentError::QualificationCarrier(domain)
        )) if domain == StructuralDomainId::new(7).unwrap()
    ));
}

#[test]
fn qualified_nested_referent_resolves_and_retires_but_out_of_range_rejects() {
    use terminal_psi::{
        StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let mut placed = module(Vec::new());
    placed
        .placed_view_inputs
        .push(placed_view_row(placed.entry, 0));
    let array_type = StructuralTypeId::new(2).unwrap();
    let element_type = StructuralTypeId::new(3).unwrap();
    placed.structural_types[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
            identity: "cells".to_owned(),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(array_type),
        }],
    };
    placed.structural_types.extend([
        StructuralTypeDeclaration {
            id: array_type,
            identity: "Cells".to_owned(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: element_type,
            identity: "Cell".to_owned(),
            shape: StructuralTypeShape::Record { fields: Vec::new() },
        },
    ]);
    let mut supply = establishment(placed.placed_view_inputs[0].clone(), 0xC0FFEE);
    supply.referent.path = vec![
        StructuralPathSegment::Field("cells".to_owned()),
        StructuralPathSegment::FixedIndex(1),
    ];
    let mut execution = TerminalExecution::start_verified_module(
        placed.clone(),
        &[],
        &[],
        &[],
        &[],
        std::slice::from_ref(&supply),
        None,
    )
    .expect("declared field and in-range element bind the qualified root's loan");
    assert_eq!(execution.placed_view_occurrences.len(), 1);
    assert!(matches!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut AcceptTerminalEffects
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    ));
    assert!(execution.placed_view_occurrences.is_empty());
    supply.referent.path[1] = StructuralPathSegment::FixedIndex(2);
    assert!(matches!(
        TerminalExecution::start_verified_module(placed, &[], &[], &[], &[], &[supply], None),
        Err(TerminalInterpretError::PlacedViewReferent(
            terminal_semantics::PlacedViewReferentError::RangeUnresolved
        ))
    ));
}
