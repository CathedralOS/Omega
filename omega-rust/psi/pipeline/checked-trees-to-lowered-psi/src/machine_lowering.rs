//! Machine lowering: the selected checked machine to unsealed, target-neutral Psi.
//!
//! [`lower_machine`] and [`lower_machine_by_symbol`] select one checked Terminal
//! machine, dispatch it through [`crate::machine_lowering::machine_dispatch`] to the plan family
//! that owns its shape, then sequence the work every selected module still
//! needs: retained custody, float-meaning projections, evidence and proof
//! recursion, proof-only quotient correspondence rows, operand proof
//! completion or module validation, and the debug companion. [`lower_bounded_callback_identity_machine`] is the deliberately
//! narrower callback-body entrance. Unsupported source constructs fail closed.

pub(crate) mod bounded_callbacks;
pub(crate) mod conformance_publication;
pub(crate) mod debug_map;
pub(crate) mod float_meanings;
pub(crate) mod guarded_exits;
pub(crate) mod machine_dispatch;
pub(crate) mod reborrow_handoffs;
pub(crate) mod specialization_commitments;

pub use bounded_callbacks::lower_bounded_callback_identity_machine;

use checked_trees::{CheckedTerminalSignatureEligibility, CheckedTrees};
use lowered_psi::LoweredPsi;

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::conformance_publication::publish_selected_conformance_applications;
use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::float_meanings::retain_float_meanings;
use crate::machine_lowering::guarded_exits::{
    GuardedExitAdmission, reject_unguarded_outcome_guarantees,
};
use crate::machine_lowering::machine_dispatch::{
    lower_selected_machine, select_terminal_machine, select_terminal_machine_by_symbol,
};
use crate::machine_lowering::reborrow_handoffs::retain_admitted_reborrow_root_handoffs;
use crate::machine_lowering::specialization_commitments::selected_closure_specializations;
use crate::producer_result::{DebugPublication, LoweredSelectedMachine, OperandProofCompletion};
use crate::proofs::evidence_lowering::lower_and_install_evidence_artifacts;
use crate::proofs::operation_proofs::finalize_operation_proofs;
use crate::proofs::proof_recursion::lower_and_install_proof_recursion;
use crate::proofs::quotient_correspondence::retain_checked_quotient_correspondences;
use crate::retention::placed_view_inputs::retain_selected_placed_view_inputs;
use crate::retention::{
    closed_reach_applications, operation_crash_contracts, reborrow_restored_call_use,
    retained_borrow_custody, suspension_call_plan,
};
use crate::unit::attached_unit;

/// Lower a selected checked machine and its required source closure.
/// Preserve source custody, install evidence, validate the completed module,
/// and attach debug companions before returning unsealed Psi.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine_name: &str,
) -> Result<LoweredPsi, LoweringError> {
    let selection = select_terminal_machine(checked, machine_name)?;
    lower_terminal_selection(checked, selection)
}

/// Lower the machine whose exact checked symbol was selected upstream.
///
/// Build product operands resolve their implementation lexically and retain
/// the exact machine symbol, so production must rejoin that symbol rather than
/// a qualified name another package could also declare. This entry point
/// shares every post-selection obligation with [`lower_machine`]; only the
/// lookup key differs.
pub fn lower_machine_by_symbol(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<LoweredPsi, LoweringError> {
    let selection = select_terminal_machine_by_symbol(checked, machine)?;
    lower_terminal_selection(checked, selection)
}

/// The checked `let`/`boundary let` mathematical declarations carry no
/// Terminal Psi evidence encoding yet, so no production route may emit a
/// module that omits them: every public lowering entrance refuses the roster
/// loudly until PROOF-CONTRACT-MIGRATION consumes it downstream.
pub(crate) fn reject_mathematical_declarations(
    checked: &CheckedTrees,
) -> Result<(), LoweringError> {
    if checked.facts.proof.mathematical_declarations.is_empty() {
        return Ok(());
    }
    unsupported(
        "mathematical `let`/`boundary let` declarations check onto checked trees, but \
         no Terminal evidence encoding carries them yet (PROOF-CONTRACT-MIGRATION)",
    )
}

/// Source custody may select different claim lineages at different normal
/// exits. Terminal's current structural return transfers carry one lineage,
/// not that checked choice. Refuse only demanded owners: an unused source
/// allocator must not prevent publishing an unrelated machine.
pub(crate) fn reject_conditional_claim_joins(
    checked: &CheckedTrees,
    source_machines: &[symbols::SymbolHandle],
) -> Result<(), LoweringError> {
    if checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .any(|(_, event)| {
            source_machines.contains(&event.machine_symbol)
                && matches!(
                    event.provenance,
                    language_semantics::PermissionProvenance::Joined { .. }
                )
        })
        || checked
            .facts
            .flow
            .ownership
            .claim_join_receipts
            .iter()
            .any(|(_, receipt)| source_machines.contains(&receipt.machine_symbol))
    {
        return unsupported(
            "conditional result custody requires Terminal exit-alternative correspondence",
        );
    }
    Ok(())
}

fn lower_terminal_selection(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
) -> Result<LoweredPsi, LoweringError> {
    reject_mathematical_declarations(checked)?;
    reject_conditional_claim_joins(checked, &[selection.machine])?;
    operation_crash_contracts::reject_unjoinable_named_sites(checked, selection.machine)?;
    // Custody gates order deliberately: this program-level check runs before
    // per-machine lowering, so a custody-carrying unit-effects plan whose
    // typed machine/state rows are missing reports "direct Unit parameter
    // plan has no exact typed machine" ahead of the scalar source-custody
    // gates in expression_preparation/source_custody. Both refusals name the
    // same root cause (checked facts alone cannot supply authored occurrence
    // custody); the earlier one fires first on a fully checked program only
    // when the typed frontend was dropped. terminal_psi_source pins the order.
    attached_unit::validate_direct_unit_parameter_custody(checked)?;
    let exit_admission = GuardedExitAdmission::for_entry(checked, selection.machine);
    reject_unguarded_outcome_guarantees(
        checked,
        &[selection.machine],
        selection.machine,
        &exit_admission,
        false,
    )?;
    let LoweredSelectedMachine {
        terminal: mut lowered,
        completion,
        source_machines,
        source_mapping,
    } = lower_selected_machine(checked, selection)?;
    reject_conditional_claim_joins(checked, &source_machines)?;
    let has_exact_source_owners = source_mapping.exact_owners().is_some();
    let entry_source = [(selection.machine, lowered.semantic_module.entry)];
    retain_admitted_reborrow_root_handoffs(
        checked,
        &source_machines,
        source_mapping.exact_owners().unwrap_or(&entry_source),
        &mut lowered.semantic_module,
    )?;
    let projection_sources =
        source_mapping.projection_sources(&lowered, selection.machine, &source_machines)?;
    validation::validate_checked_machine_specialization_commitments(
        checked,
        &selected_closure_specializations(checked, &source_machines, &lowered),
    )
    .map_err(LoweringError::Unsupported)?;
    closed_reach_applications::retain_closed_reach_applications(
        checked,
        &projection_sources,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retain_selected_placed_view_inputs(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &mut lowered.semantic_module,
    )?;
    reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
        checked,
        selection.machine,
        lowered.semantic_module.entry,
        &lowered.source_call_occurrences,
        &lowered.semantic_module.machines,
        &mut lowered.semantic_module.reborrow_restored_call_uses,
    )?;
    suspension_call_plan::retain_suspension_call_plans(
        checked,
        &source_machines,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retained_borrow_custody::retain_foreign_borrow_custodies(
        checked,
        &mut lowered.semantic_module,
    )?;
    // Selected operator invocations carry their crash contract beside the
    // exact emitted operation; a crash-qualified use the closure cannot join
    // to one operation fails closed here rather than lowering crash-free.
    operation_crash_contracts::retain_operation_crash_contracts(
        checked,
        &source_machines,
        &mut lowered,
    )?;
    reject_unguarded_outcome_guarantees(
        checked,
        &source_machines,
        selection.machine,
        &exit_admission,
        true,
    )?;
    publish_selected_conformance_applications(
        checked,
        selection.machine,
        &completion.conformances,
        has_exact_source_owners,
        &projection_sources,
        &source_machines,
        &mut lowered,
    )?;
    retain_float_meanings(checked, &projection_sources, &mut lowered)?;
    lower_and_install_evidence_artifacts(checked, selection.machine, &mut lowered)?;
    lower_and_install_proof_recursion(
        checked,
        &source_machines,
        &mut lowered.semantic_module,
        &mut lowered.proof_bundle,
    )?;
    // Proof-only quotient correspondence rows join module identity before
    // validation; a nonempty table is then refused by the execution gate below
    // until executable quotient lowering exists.
    retain_checked_quotient_correspondences(checked, &mut lowered.semantic_module)?;
    // Unit closures can be provisional inputs to cleanup/borrow assembly.
    // Discharge operand obligations only after the selected module is complete.
    if completion.operands == OperandProofCompletion::Finalize {
        finalize_operation_proofs(&mut lowered)?;
    } else if lowered
        .semantic_module
        .machines
        .iter()
        .any(|machine| machine.ranked_scc.is_some())
    {
        terminal_verifier::validate_module_for_interpretation(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    } else {
        terminal_verifier::validate_module(&lowered.semantic_module)
            .map_err(LoweringError::InvalidTerminalModule)?;
    }
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && completion.debug == DebugPublication::FromCheckedPlan
    {
        checked
            .facts
            .flow
            .terminal_debug
            .for_machine(selection.machine)
            .map(|plan| build_debug_map(plan, &lowered.semantic_module))
            .transpose()?
    } else {
        None
    };
    Ok(lowered)
}
