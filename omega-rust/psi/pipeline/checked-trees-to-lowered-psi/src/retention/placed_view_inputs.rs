//! Placed-view input custody retained on the selected Terminal machine.
//!
//! Every checked placed-view input of the selected source machine is rejoined
//! to its hermetic declaration identities; disagreeing custody for one
//! coordinate fails closed rather than being overwritten.

use checked_trees::{CheckedPlacedViewInput, CheckedTrees};
use semantic_vocabulary::MachineId;
use terminal_psi::{StructuralAccess, TerminalModule, TerminalPlacedViewInput};

use crate::lowering_error::{LoweringError, unsupported};

pub(crate) fn retain_selected_placed_view_inputs(
    checked: &CheckedTrees,
    source_machine: symbols::SymbolHandle,
    terminal_machine: MachineId,
    module: &mut TerminalModule,
) -> Result<(), LoweringError> {
    for input in checked
        .facts
        .placed_view_inputs
        .iter()
        .filter(|input| input.machine == source_machine)
    {
        let lowered = lower_placed_view_input(checked, input, terminal_machine)?;
        if let Some(existing) = module.placed_view_inputs.iter().find(|existing| {
            (
                existing.machine,
                &existing.source_state_identity,
                existing.position,
            ) == (
                lowered.machine,
                &lowered.source_state_identity,
                lowered.position,
            )
        }) {
            if existing != &lowered {
                return unsupported("placed-view input custody disagrees across Terminal lowering");
            }
        } else {
            module.placed_view_inputs.push(lowered);
        }
    }
    module.placed_view_inputs.sort();
    Ok(())
}

pub(crate) fn lower_placed_view_input(
    checked: &CheckedTrees,
    input: &CheckedPlacedViewInput,
    machine: MachineId,
) -> Result<TerminalPlacedViewInput, LoweringError> {
    let identity = |symbol, missing| {
        checked
            .typed
            .normalized_hermetic_symbol_identity(symbol)
            .map_err(|_| LoweringError::Unsupported(missing))
    };
    let access = match input.reference_access {
        language_core::ReferenceAccess::Shared => StructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => StructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => StructuralAccess::WriteOnlyBorrow,
    };
    let policy_identity = identity(
        input.policy,
        "placed-view policy has no hermetic declaration identity",
    )?;
    let schema_identity = identity(
        input.schema,
        "placed-view schema has no hermetic declaration identity",
    )?;
    let view_identity =
        terminal_psi::canonical_placed_view_identity(&policy_identity, &schema_identity);
    Ok(TerminalPlacedViewInput {
        machine,
        position: input.position,
        source_machine_identity: identity(
            input.machine,
            "placed-view consumer has no hermetic declaration identity",
        )?,
        source_state_identity: identity(
            input.state,
            "placed-view state has no hermetic declaration identity",
        )?,
        source_parameter_identity: identity(
            input.parameter,
            "placed-view parameter has no hermetic declaration identity",
        )?,
        access,
        binding_is_const: input.binding_is_const,
        binding_is_mutable: input.binding_is_mutable,
        view_identity,
        policy_identity,
        policy_plan_machine_identity: identity(
            input.policy_plan_machine,
            "placed-view plan machine has no hermetic declaration identity",
        )?,
        schema_identity,
        placement_report_fingerprint: input.placement.identity().compatibility_fingerprint(),
        placement_commitment: input.placement.content_interpretation().commitment(),
    })
}
