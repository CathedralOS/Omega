//! Normal-call transfer of reference slots. A type supplies the slot inventory;
//! the complete body supplies its possible exit contents. Identity rows mean
//! incoming contents survive that route, not that the carrier is its backing.
//! Unknown new contents, local backing escapes and recursive summaries fail
//! closed. Rows retain each slot's declared access alongside provenance;
//! checked stores still own permission and predicate authority.
//!
//! Slice inventories retain incoming defaults plus exact slot replacements.
//! Normal-exit summaries refine those defaults across all reachable exits before
//! joining their origins. A branch which never writes therefore retains its
//! incoming alternative; an untouched sibling keeps its original descriptor.
//! Type-default selectors are not executable indices. A runtime-selected store
//! publishes one possible destination region, retaining old contents, rather
//! than exporting its expression identity or duplicating the transfer per slot.

mod slot_relations;
use slot_relations::{DestinationInventory, SlotInventory};
pub(super) use slot_relations::{SlotEffect, SlotRelationTransfer, SlotSource};
pub use slot_relations::{
    StoredReferenceCallFailure, StoredReferenceEffect, StoredReferenceSource,
};
mod slot_inventory;
use slot_inventory::destination_inventory;
pub(crate) use slot_inventory::provider_completion_covers_writable_slots;
pub(super) use slot_inventory::{provider_output_relation_required, seed_incoming_slots};
mod normal_exits;
pub(super) use normal_exits::state_effects;
mod instantiation;
use instantiation::{aggregate_argument_sources, argument_origin, destination_source, instantiate};
mod transfers;
pub(super) use transfers::{apply, public_effects};

mod expression_effects;
mod opaque_outputs;
pub(super) mod retention;
use expression_effects::CompletedArguments;
pub(super) use expression_effects::statement_effects;

use super::caller_aliases::{LocalWriteOrigin, LocalWriteSource};
use super::place_paths::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
use super::state_write_walk::{CollectedStatementPrefix, stored_reference_transfer};
use super::stored_origins::{self, StoredLocalOrigins, StoredWriteOrigin};
use super::{FrameInference, Machine, SymbolHandle, TableCall, TopLevelSymbols};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

fn call_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    call: &TableCall,
    completed: &CompletedArguments,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    invocation_effects(
        program,
        machine,
        caller_state,
        call.target_symbol,
        ExpressionHandle::invalid(),
        completed,
        call.machine_arguments.is_empty(),
        context,
        symbols,
        inference,
    )
}

fn invocation_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    target: SymbolHandle,
    call_expression: ExpressionHandle,
    completed: &CompletedArguments,
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let arguments = &completed.expressions;
    let Some((callee, state)) = super::machine_state_by_symbol(program, target) else {
        if let Some((definition, signature)) =
            crate::selected_requirement_signature(program, target)
            && definition.is_boundary
        {
            let signature = if call_expression.is_valid() {
                let call = program.expression_table.call_node(call_expression)?;
                if call.target_symbol != target {
                    return None;
                }
                program.requirement_call_signature(machine.symbol, call)?
            } else {
                signature
            };
            let parameters = program.state_signature_parameters(signature);
            return opaque_outputs::effects(
                program, machine, parameters, completed, context, symbols, inference,
            );
        }
        // A requirement or intrinsic operation has no checked body to replay.
        // It can still install a reference slot only through an argument that
        // lends exclusive storage; value and shared arguments leave every
        // stored slot as it was. An argument of unknown type may lend.
        return arguments
            .iter()
            .all(|argument| !lends_exclusive_storage(program, machine, caller_state, *argument))
            .then(|| SlotRelationTransfer::complete(Vec::new()));
    };
    let parameters = program.state_parameters(state);
    if (parameters.len() != arguments.len())
        || !machine_arguments_empty
        || !program.machine_type_parameters(callee).is_empty()
    {
        return None;
    }
    if matches!(
        callee.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
            | language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) {
        return opaque_outputs::effects(
            program, machine, parameters, completed, context, symbols, inference,
        );
    }
    // An authenticated empty writable-slot inventory needs no body summary.
    // This also covers portable target-family heads: their members cannot
    // replace caller descriptors without a formal that lends such storage.
    if destination_inventory(program, state)?.is_empty() {
        return Some(SlotRelationTransfer::complete(Vec::new()));
    }
    if !callee.body_is_present || !callee.supply_mode.is_checked_body() {
        return None;
    }
    let relative = state_effects(program, callee, state, symbols, inference)?;
    instantiate(
        program, machine, parameters, completed, &relative, context, symbols, inference,
    )
    .map(SlotRelationTransfer::complete)
}

fn lends_exclusive_storage(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    argument: ExpressionHandle,
) -> bool {
    if let typed_trees::expression::ExpressionNode::Borrow(borrow) =
        program.expression_table.expression(argument)
    {
        return borrow.access.is_exclusive();
    }
    crate::expression_result_type_reference(program, machine, state, argument).is_none_or(
        |reference| {
            crate::reference_shell(program, reference)
                .is_some_and(|shell| shell.access.is_exclusive())
        },
    )
}

fn path(program: &dyn crate::CheckingInput, source: &FrameSourcePlace) -> String {
    super::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    )
    .0
}
