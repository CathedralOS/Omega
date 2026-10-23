//! Lower the checked placed-view inputs one closed machine carries.
//!
//! `unit::attached_unit::lower_unit_closure` assembles the one module every
//! plan family publishes and lowers every checked placed-view input of a
//! machine it closes over, joining each row to its hermetic declaration
//! identities. This module owns that join; there is no second publication of
//! the same rows to reconcile.

use checked_trees::{CheckedPlacedViewInput, CheckedTrees};
use semantic_vocabulary::MachineId;
use terminal_psi::{StructuralAccess, TerminalPlacedViewInput};

use crate::lowering_error::LoweringError;

pub(crate) fn lower_placed_view_input(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
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
    let source_machine =
        checked
            .typed
            .machine_by_symbol(input.machine)
            .ok_or(LoweringError::Unsupported(
                "placed-view input lost its source machine",
            ))?;
    let state = typed_trees::state::state_by_symbol(
        checked.typed.machine_states(source_machine),
        input.state,
    )
    .ok_or(LoweringError::Unsupported(
        "placed-view input lost its source state",
    ))?;
    let parameters = checked.typed.state_parameters(state);
    let parameter = parameters
        .get(input.position as usize)
        .filter(|parameter| parameter.symbol == input.parameter)
        .ok_or(LoweringError::Unsupported(
            "placed-view input lost its exact source formal",
        ))?;
    // Join the actual checked structural telescope, rather than duplicating
    // its scalar/erased classifier. Only an executable machine's initial state
    // owns an input boundary; ordinary block loans transport later states.
    let entry = checked
        .facts
        .flow
        .terminal_unit_effects
        .callable_entry_for_machine(input.machine)
        .filter(|entry| entry.state == input.state)
        .ok_or(LoweringError::Unsupported(
            "placed-view input has no exact checked callable entry",
        ))?;
    let runtime_position = entry
        .structural_parameters
        .iter()
        .position(|parameter| parameter.position == input.position)
        .and_then(|position| u32::try_from(position).ok())
        .ok_or(LoweringError::Unsupported(
            "placed-view input has no checked structural formal",
        ))?;
    let carrier =
        validation::structural_type_identity(&checked.typed, parameter.type_reference, &[], &[]);
    let carrier_text =
        checked
            .typed
            .type_identity_text(carrier)
            .ok_or(LoweringError::Unsupported(
                "placed-view formal lost its structural carrier identity",
            ))?;
    let carrier_identity = identities.intern(&carrier_text);
    Ok(TerminalPlacedViewInput {
        machine,
        position: input.position,
        runtime_position,
        carrier_identity,
        source_machine_identity: crate::machine_lowering::declarations::nominal(
            identities,
            checked,
            exact_toolchain_sources,
            input.machine,
        )
        .map_err(LoweringError::Unsupported)?
        .identity(),
        source_state_identity: crate::machine_lowering::declarations::nominal(
            identities,
            checked,
            exact_toolchain_sources,
            input.state,
        )
        .map_err(LoweringError::Unsupported)?
        .identity(),
        source_parameter_identity: identities.intern(&identity(
            input.parameter,
            "placed-view parameter has no hermetic declaration identity",
        )?),
        access,
        binding_is_const: input.binding_is_const,
        binding_is_mutable: input.binding_is_mutable,
        view_identity: identities.intern(&view_identity),
        policy_identity: identities.intern(&policy_identity),
        policy_plan_machine_identity: identities.intern(&identity(
            input.policy_plan_machine,
            "placed-view plan machine has no hermetic declaration identity",
        )?),
        schema_identity: identities.intern(&schema_identity),
        placement_report_fingerprint: input.placement.identity().compatibility_fingerprint(),
        placement_commitment: input.placement.content_interpretation().commitment(),
    })
}
