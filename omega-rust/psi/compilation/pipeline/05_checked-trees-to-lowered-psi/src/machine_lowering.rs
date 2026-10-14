//! Machine lowering: the selected checked machine to unsealed, target-neutral Psi.
//!
//! [`lower_machine`] selects one checked Terminal machine by
//! [`TerminalMachineSelection`], dispatches it through
//! [`crate::machine_lowering::machine_dispatch`] to the plan family that owns
//! its shape, then closes provider installation entries and sequences the work
//! every selected module still needs: retained custody, float-meaning projections, evidence and proof
//! recursion, checked quotient correspondence bindings, operand proof
//! completion or module validation, and the debug companion.
//! Callback selections join the same program's retained roots. Each body retains its portable
//! checked declaration, so consumers need no source-to-Terminal receipt.
//! State-exit loan closure stays in source checking: it has no executable
//! Terminal operation and needs no serialized lineage certificate. Restored
//! calls below still retain their operation-specific source relationships.
//! Unsupported source constructs fail closed.

mod boundary_applications;
mod callback_applications;
pub(crate) mod debug_map;
pub(crate) mod declarations;
pub(crate) mod float_meanings;
pub(crate) mod guarded_exits;
pub(crate) mod machine_dispatch;

use checked_trees::{CheckedTerminalSignatureEligibility, CheckedTrees};
use lowered_psi::LoweredPsi;

use crate::lowering_error::{LoweringError, unsupported};
use crate::machine_lowering::debug_map::build_debug_map;
use crate::machine_lowering::float_meanings::retain_float_meanings;
use crate::machine_lowering::guarded_exits::reject_unguarded_outcome_guarantees;
use crate::machine_lowering::machine_dispatch::{
    TerminalMachineSelection, lower_selected_machine, select_terminal_machine,
};
use crate::producer_result::{DebugPublication, LoweredSelectedMachine, LoweringCompletion};
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
    program_roots: crate::TerminalProgramRoots<'_>,
) -> Result<LoweredPsi, LoweringError> {
    if checked
        .facts
        .integer_range_obligations
        .iter()
        .any(checked_trees::IntegerRangeObligation::is_pending)
    {
        return unsupported(
            "integer range obligations require target closure before Terminal production",
        );
    }
    // A shared source judgment may carry target-dependent proof conditions.
    // Only target closure consumes those obligations; direct/package callers
    // cannot publish conditional call evidence as unconditional Terminal code.
    if checked
        .facts
        .constant_predicate_obligations
        .iter()
        .any(checked_trees::ConstantPredicateObligation::is_pending)
    {
        return unsupported(
            "constant proof predicates require target closure before Terminal production",
        );
    }
    // One builder owns the complete closure's identities. Body construction
    // snapshots only a prefix: declaration, evidence and application retention
    // can intern more rows. Seal the complete table after those producers, so
    // every durable index resolves in the module that owns it.
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    // Nominal callbacks are checked callable roots, even though their foreign
    // invocation is not an ordinary source call edge. Retain them once beside
    // program entries; native ABI adapters select these exact bodies later.
    let mut roots = program_roots.machines.to_vec();
    roots.extend(
        checked
            .facts
            .nominal_machine_uses
            .uses
            .iter()
            .filter(|usage| usage.callback_entry.is_some())
            .map(|usage| usage.selected_machine),
    );
    roots.sort_unstable_by_key(|root| (root.arena_index(), root.generation()));
    roots.dedup();
    let program_roots = crate::TerminalProgramRoots {
        machines: &roots,
        boundaries: program_roots.boundaries,
    };
    let selection = select_terminal_machine(checked, machine)?;
    let (mut lowered, completion) = lower_terminal_selection(
        &mut identities,
        checked,
        selection,
        exact_toolchain_sources,
        program_roots,
    )?;
    boundary_applications::retain(
        &mut identities,
        checked,
        exact_toolchain_sources,
        &mut lowered,
    )
    .map_err(LoweringError::Unsupported)?;
    callback_applications::retain(
        &mut identities,
        checked,
        exact_toolchain_sources,
        &mut lowered,
    )
    .map_err(LoweringError::Unsupported)?;
    lowered.semantic_module.identities = identities.build();
    // Debug identity binds the whole semantic module, including retained
    // boundary/callback applications and the final interned identity table.
    // Construct it only after those producers finish; stamping the body's
    // earlier table prefix makes even a constant entry's companion stale.
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

/// The exact source declaration designates this callable's cleanup role;
/// merely being attached to the same type or receiving its loan is insufficient.
pub(crate) fn nominal_cleanup_subject(
    checked: &CheckedTrees,
    source: symbols::SymbolHandle,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<Option<semantic_vocabulary::StructuralTypeId>, LoweringError> {
    let rendered = type_ids
        .iter()
        .map(|(identity, id)| {
            (
                checked.type_identity_text(*identity).unwrap_or_default(),
                *id,
            )
        })
        .collect::<Vec<_>>();
    nominal_cleanup_subject_from_types(
        checked,
        source,
        rendered
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
        .find(|(candidate, _)| {
            *candidate == checked.type_identity_text(identity).unwrap_or_default()
        })
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
            module.structural_types.iter().map(|declaration| {
                (
                    module.identities.text(declaration.identity).unwrap_or(""),
                    declaration.id,
                )
            }),
        )?;
        // Nominal ownership survives elision of an unused hook argument; it
        // does not manufacture an implicit receiver or storage declaration.
        machine.attachment = machine.attachment.or(expected);
        machine.nominal_cleanup_subject = expected;
    }
    Ok(())
}

fn lower_terminal_selection(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    selection: &checked_trees::CheckedTerminalMachineSelection,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    program_roots: crate::TerminalProgramRoots<'_>,
) -> Result<(LoweredPsi, LoweringCompletion), LoweringError> {
    let LoweredSelectedMachine {
        terminal: mut lowered,
        completion,
        source_machine_ids,
    } = lower_selected_machine(
        identities,
        checked,
        selection,
        program_roots,
        exact_toolchain_sources,
    )?;
    if lowered.semantic_module.target_machine_families.len()
        != crate::unit::attached_unit::target_families::runtime_families(checked)?.len()
    {
        return unsupported("entry lowering did not retain the complete target-family catalog");
    }
    let source_machines = source_machine_ids
        .iter()
        .map(|(source, _)| *source)
        .collect::<Vec<_>>();
    declarations::retain(
        identities,
        checked,
        exact_toolchain_sources,
        &source_machine_ids,
        &mut lowered.semantic_module,
    )
    .map_err(LoweringError::Unsupported)?;
    retain_nominal_cleanup_roles(checked, &source_machine_ids, &mut lowered.semantic_module)?;
    closed_reach_applications::retain_closed_reach_applications(
        identities,
        checked,
        &source_machine_ids,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    for (source_machine, terminal_machine) in &source_machine_ids {
        retain_selected_placed_view_inputs(
            identities,
            checked,
            exact_toolchain_sources,
            *source_machine,
            *terminal_machine,
            &mut lowered.semantic_module,
        )?;
        reborrow_restored_call_use::retain_selected_reborrow_restored_call_uses(
            identities,
            checked,
            *source_machine,
            *terminal_machine,
            &lowered.source_call_occurrences,
            &lowered.semantic_module.machines,
            &mut lowered.semantic_module.reborrow_restored_call_uses,
        )?;
    }
    suspension_call_plan::retain_suspension_call_plans(
        identities,
        checked,
        &source_machines,
        &lowered.source_call_occurrences,
        &mut lowered.semantic_module,
    )?;
    retained_borrow_custody::retain_foreign_borrow_custodies(
        identities,
        checked,
        exact_toolchain_sources,
        &mut lowered.semantic_module,
    )?;
    // Selected operator invocations carry their crash contract beside the
    // exact emitted operation; a named invocation's carrier is its own
    // boundary call's declaration, verified through the same projection
    // sources. A crash-qualified use the closure cannot join fails closed
    // here rather than lowering crash-free.
    operation_crash_contracts::retain_operation_crash_contracts(
        identities,
        checked,
        &source_machines,
        &source_machine_ids,
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
        identities,
        checked,
        &source_machines,
        &mut lowered.semantic_module,
    )?;
    crate::unit::dynamic_dispatch::lower_dynamic_dispatch_catalog(
        identities,
        checked,
        &source_machines,
        &mut lowered.semantic_module,
    )?;
    retain_float_meanings(identities, checked, &source_machine_ids, &mut lowered)?;
    lower_and_install_evidence_artifacts(identities, checked, &source_machine_ids, &mut lowered)?;
    // Copies emitted while composing values can allocate after frame reservation.
    // Retain their ordinary place declarations before publishing the module;
    // quotient adaptations keep their separate exact occurrence licenses.
    crate::emission::operation_emission::place_declarations::reconcile_leaf_copy_place_declarations(
        &mut lowered.semantic_module,
    );
    Ok((lowered, completion))
}
