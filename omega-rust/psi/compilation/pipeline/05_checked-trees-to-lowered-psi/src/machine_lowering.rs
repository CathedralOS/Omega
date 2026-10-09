//! Machine lowering: the selected checked machine to unsealed, target-neutral Psi.
//!
//! [`lower_machine`] selects one checked Terminal machine by
//! [`TerminalMachineSelection`], dispatches it through
//! [`crate::machine_lowering::machine_dispatch`] to the plan family that owns
//! its shape, then closes provider installation entries and sequences the work
//! every selected module still needs: retained custody, float-meaning projections, evidence and proof
//! recursion, proof-only quotient correspondence rows, operand proof
//! completion or module validation, and the debug companion.
//! A callback body is not a second entrance: it lowers through this same
//! route rooted at the callback entry, and [`callback_lowering_receipt`] then
//! joins the checked callback coordinate onto the module's entry machine.
//! Unsupported source constructs fail closed.

mod boundary_applications;
pub(crate) mod bounded_callbacks;
mod callback_applications;
pub(crate) mod debug_map;
pub(crate) mod float_meanings;
pub(crate) mod guarded_exits;
pub(crate) mod machine_dispatch;
pub(crate) mod reborrow_handoffs;

pub use bounded_callbacks::callback_lowering_receipt;

use checked_trees::{CheckedTerminalSignatureEligibility, CheckedTrees};
use lowered_psi::LoweredPsi;

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::float_meanings::retain_float_meanings;
use crate::machine_lowering::guarded_exits::reject_unguarded_outcome_guarantees;
use crate::machine_lowering::machine_dispatch::{
    TerminalMachineSelection, lower_selected_machine, select_terminal_machine,
};
use crate::machine_lowering::reborrow_handoffs::retain_admitted_reborrow_root_handoffs;
use crate::producer_result::{DebugPublication, LoweredSelectedMachine};
use crate::proofs::evidence_lowering::lower_and_install_evidence_artifacts;
use crate::retention::placed_view_inputs::retain_selected_placed_view_inputs;
use crate::retention::{
    closed_reach_applications, operation_crash_contracts, reborrow_restored_call_use,
    retained_borrow_custody, suspension_call_plan,
};
use crate::unit::attached_unit;

/// Lower the selected checked machine and its required source closure.
/// Preserve source custody, install evidence, validate the completed module,
/// and attach debug companions before returning unsealed Psi.
pub fn lower_machine(
    checked: &CheckedTrees,
    machine: TerminalMachineSelection<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<LoweredPsi, LoweringError> {
    let selection = select_terminal_machine(checked, machine)?;
    let mut lowered = lower_terminal_selection(checked, selection)?;
    boundary_applications::retain(checked, exact_toolchain_sources, &mut lowered)
        .map_err(LoweringError::Unsupported)?;
    callback_applications::retain(checked, &mut lowered).map_err(LoweringError::Unsupported)?;
    Ok(lowered)
}

/// The exact source declaration designates this callable's cleanup role;
/// merely being attached to the same type or receiving its loan is insufficient.
pub(crate) fn nominal_cleanup_subject(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    type_ids: &[(String, semantic_vocabulary::StructuralTypeId)],
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    nominal_cleanup_subject_from_types(
        checked,
        source,
        type_ids
            .iter()
            .map(|(identity, id)| (identity.as_str(), *id)),
    )
}

fn nominal_cleanup_subject_from_types<'types>(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    mut types: impl Iterator<Item = (&'types str, semantic_vocabulary::StructuralTypeId)>,
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    let Some(owner) = validation::nominal_cleanup_owner(&checked.typed, source) else {
        return Ok(None);
    };
    let machine = checked.machine(source).ok_or(LoweringError::Unsupported(
        "cleanup declaration lost its exact source",
    ))?;
    let data = checked
        .data_definition_by_symbol(owner)
        .ok_or(LoweringError::Unsupported(
            "cleanup declaration lost its exact owner",
        ))?;
    let identity = if machine.attached_data_application.is_valid() {
        checked.normalized_type_identity(machine.attached_data_application)
    } else if let Some(application) = data.generic_instance {
        checked.normalized_type_identity(application)
    } else {
        checked
            .normalized_nominal_type_identity(owner)
            .ok_or(LoweringError::Unsupported(
                "cleanup owner has no normalized nominal identity",
            ))?
    };
    let (_, subject) = types
        .find(|(candidate, _)| *candidate == identity.as_str())
        .ok_or(LoweringError::Unsupported(
            "owner-designated cleanup lost its exact structural declaration",
        ))?;
    Ok(Some(subject))
}

fn retain_nominal_cleanup_roles(
    checked: &CheckedTrees,
    owners: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
    module: &mut terminal_psi::TerminalModule,
) -> Result<(), LoweringError> {
    for (source, identity) in owners {
        let mut matches = module
            .machines
            .iter_mut()
            .filter(|machine| machine.id == *identity);
        let machine = matches.next().ok_or(LoweringError::Unsupported(
            "cleanup source mapping lost its emitted machine",
        ))?;
        if matches.next().is_some() {
            return unsupported("cleanup source mapping repeats its emitted machine");
        }
        let expected = nominal_cleanup_subject_from_types(
            checked,
            *source,
            module
                .structural_types
                .iter()
                .map(|declaration| (declaration.identity.as_str(), declaration.id)),
        )?;
        // Nominal ownership survives elision of an unused hook argument; it
        // does not manufacture an implicit receiver or storage declaration.
        machine.attachment = machine.attachment.or(expected);
        machine.nominal_cleanup_subject = expected;
    }
    Ok(())
}

fn lower_terminal_selection(
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
) -> Result<LoweredPsi, LoweringError> {
    let LoweredSelectedMachine {
        terminal: mut lowered,
        completion,
        source_machines,
        source_mapping,
    } = lower_selected_machine(checked, selection)?;
    if lowered.semantic_module.target_machine_families.len()
        != checked
            .typed
            .target_machine_families
            .iter()
            .filter(|family| family.purpose == typed_trees::machine::MachinePurpose::Runtime)
            .count()
    {
        return unsupported("entry lowering did not retain the complete target-family catalog");
    }
    let entry_source = [(selection.machine, lowered.semantic_module.entry)];
    retain_nominal_cleanup_roles(
        checked,
        source_mapping.exact_owners().unwrap_or(&entry_source),
        &mut lowered.semantic_module,
    )?;
    retain_admitted_reborrow_root_handoffs(
        checked,
        &source_machines,
        source_mapping.exact_owners().unwrap_or(&entry_source),
        &mut lowered.semantic_module,
    )?;
    let projection_sources = source_mapping.projection_sources(&lowered, selection.machine)?;
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
    // exact emitted operation; a named invocation's carrier is its own
    // boundary call's declaration, verified through the same projection
    // sources. A crash-qualified use the closure cannot join fails closed
    // here rather than lowering crash-free.
    operation_crash_contracts::retain_operation_crash_contracts(
        checked,
        &source_machines,
        &projection_sources,
        &mut lowered,
    )?;
    // Guarded guarantees are admitted from the finished closure: whether the
    // entry returns one exact payloadless case is a fact of its lowered body,
    // whichever producer emitted it.
    reject_unguarded_outcome_guarantees(
        checked,
        &source_machines,
        selection.machine,
        &lowered.semantic_module,
    )?;
    crate::retention::conformance_applications::lower_closed_conformance_applications(
        checked,
        &source_machines,
        &mut lowered.semantic_module,
    )?;
    crate::unit::dynamic_dispatch::lower_dynamic_dispatch_catalog(
        checked,
        &source_machines,
        &mut lowered.semantic_module,
    )?;
    retain_float_meanings(checked, &projection_sources, &mut lowered)?;
    lower_and_install_evidence_artifacts(checked, selection.machine, &mut lowered)?;
    // Proof-only quotient correspondence rows join module identity before
    // validation; a nonempty table is then refused by the execution gate below
    // until executable quotient lowering exists.
    crate::emission::operation_emission::place_declarations::reconcile_leaf_copy_place_declarations(
        &mut lowered.semantic_module,
    );
    lowered.debug_map = if selection.signature == CheckedTerminalSignatureEligibility::Eligible
        && completion.debug == DebugPublication::FromCheckedPlan
    {
        let live_states = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(selection.machine)
            .map(attached_unit::unit_graph_live_states)
            .transpose()?;
        checked
            .facts
            .flow
            .terminal_debug
            .for_machine(selection.machine)
            .map(|plan| build_debug_map(plan, &lowered.semantic_module, live_states.as_deref()))
            .transpose()?
    } else {
        None
    };
    Ok(lowered)
}
