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
    StoredReferenceCallContext, StoredReferenceCallFailure, StoredReferenceEffect,
    StoredReferenceSource,
};
mod slot_inventory;
pub(crate) use slot_inventory::provider_completion_covers_writable_slots;
use slot_inventory::{destination_inventory, parameter_destination_inventory};
pub(super) use slot_inventory::{provider_output_relation_required, seed_incoming_slots};
mod normal_exits;
pub(super) use normal_exits::state_effects;
mod instantiation;
use instantiation::{aggregate_argument_sources, destination_source, instantiate};
mod transfers;
pub(super) use transfers::{apply, apply_transfer, public_effects};
mod captured_inputs;
mod case_generations;
pub(super) use case_generations::CaseStorageEffect;
mod expression_effects;
use expression_effects::SlotArgumentCapture;
pub(super) use expression_effects::call_contexts;
pub(super) use expression_effects::{
    StatementSlotTransfers, effects_at_expression, statement_effects,
};
mod invocation_contract;
mod opaque_outputs;
pub use invocation_contract::ReferenceSlotContract;
pub(super) use invocation_contract::invocation_contract;
pub(super) mod retention;

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
    captures: &[SlotArgumentCapture],
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
        super::caller_aliases::CallerWriteSite::Call(call),
        program.statement_table.expression_handles(call.arguments),
        captures,
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
    call_site: super::caller_aliases::CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    captures: &[SlotArgumentCapture],
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let mut transfer = invocation_reference_effects(
        program,
        machine,
        caller_state,
        target,
        call_expression,
        call_site,
        arguments,
        captures,
        machine_arguments_empty,
        context,
        symbols,
        inference,
    )?;
    if transfer.is_complete() {
        transfer.case_effects = case_generations::completed_effects(
            program,
            machine,
            caller_state,
            target,
            captures,
            context,
            symbols,
            inference,
        );
    }
    Some(transfer)
}

fn invocation_reference_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    target: SymbolHandle,
    call_expression: ExpressionHandle,
    call_site: super::caller_aliases::CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    captures: &[SlotArgumentCapture],
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let trace_refusal = |phase: &str| {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE stored-reference-invocation-refusal machine={:?} state={:?} target={:?} expression={:?} phase={} argument_count={} capture_count={} divergent_count={} machine_arguments_empty={}",
                machine.symbol,
                caller_state.symbol,
                target,
                call_expression,
                phase,
                arguments.len(),
                captures.len(),
                context.divergent.len(),
                machine_arguments_empty,
            );
        }
    };
    if arguments.len() != captures.len()
        || arguments
            .iter()
            .zip(captures)
            .any(|(expression, capture)| *expression != capture.expression)
    {
        trace_refusal("argument-capture-correspondence");
        return None;
    }
    // The occurrence's applied logical contract owns caller-visible access.
    // A private body's open telescope or availability cannot turn a known
    // empty writable slot frontier into an unknown descriptor mutation.
    let parameters = match call_site {
        super::caller_aliases::CallerWriteSite::Call(call)
            if !call_expression.is_valid() && call.target_symbol == target =>
        {
            crate::CallSite::Statement(call).parameters(program, machine.symbol)?
        }
        super::caller_aliases::CallerWriteSite::Expression(expression)
            if expression == call_expression =>
        {
            let call = program.expression_table.call_node(expression)?;
            if call.target_symbol != target {
                return None;
            }
            crate::CallSite::Expression { expression, call }.parameters(program, machine.symbol)?
        }
        _ => return None,
    };
    if parameters.len() != captures.len() {
        trace_refusal("parameter-capture-correspondence");
        return None;
    }
    let inventory = parameter_destination_inventory(program, parameters)?;
    // Captured machine arguments are not state operands. The state-only
    // inventory cannot classify those captures; their existing relation owner
    // must still succeed below before this invocation can be complete.
    if machine_arguments_empty && !inventory.template_dependent && inventory.slots.is_empty() {
        // This proves only that no argument can replace caller reference
        // slots. Returned descriptor identity, loan formation, qualifications,
        // scalar/content writes and normal-call permission have other owners.
        return Some(SlotRelationTransfer::complete(Vec::new()));
    }
    let Some((callee, state)) = super::machine_state_by_symbol(program, target) else {
        if let Some((definition, _)) = crate::selected_requirement_signature(program, target)
            && definition.is_boundary
        {
            return opaque_outputs::effects(
                program,
                machine,
                parameters,
                captures,
                context,
                symbols,
                call_site,
                target,
                call_expression,
            );
        }
        // A missing body cannot summarize a nonempty or symbolic frontier.
        return None;
    };
    let body_parameters = program.state_parameters(state);
    if body_parameters.len() != arguments.len()
        || !machine_arguments_empty
        || !program.machine_type_parameters(callee).is_empty()
    {
        trace_refusal("checked-callee-application");
        return None;
    }
    if matches!(
        callee.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
            | language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) {
        return opaque_outputs::effects(
            program,
            machine,
            parameters,
            captures,
            context,
            symbols,
            call_site,
            target,
            call_expression,
        );
    }
    let parameters = body_parameters;
    if !callee.body_is_present || !callee.supply_mode.is_checked_body() {
        trace_refusal("checked-body-unavailable");
        return None;
    }
    let Some(destinations_empty) =
        destination_inventory(program, state).map(|destinations| destinations.is_empty())
    else {
        trace_refusal("destination-inventory");
        return None;
    };
    if destinations_empty {
        return Some(SlotRelationTransfer::complete(Vec::new()));
    }
    let indices = captures
        .iter()
        .map(|captured| captured.inference.index_value(program, captured.expression))
        .collect::<Vec<_>>();
    let Some(mut relative) =
        inference.with_invocation(program, state, &[], &indices, |inference| {
            state_effects(program, callee, state, symbols, inference)
        })
    else {
        trace_refusal("relative-state-effects");
        return None;
    };
    rebase_output_occurrences(
        program,
        machine,
        caller_state,
        call_site,
        target,
        call_expression,
        parameters,
        &mut relative,
    )?;
    instantiate(
        program, machine, parameters, captures, &relative, context, symbols,
    )
    .map(|instantiated| SlotRelationTransfer::complete(instantiated.effects))
}

fn output_identity(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    site: super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> Option<facts::InvocationOutputIdentity> {
    let (state, _, statement_index) =
        super::caller_aliases::caller_statement_owner(program, machine, site)?;
    let mut sites =
        crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .enumerate()
            .filter(|(_, occurrence)| match (site, occurrence) {
                (
                    super::caller_aliases::CallerWriteSite::Call(expected),
                    crate::CallSite::Statement(actual),
                ) => std::ptr::eq(expected, *actual),
                (
                    super::caller_aliases::CallerWriteSite::Expression(expected),
                    crate::CallSite::Expression { expression, .. },
                ) => expected == *expression,
                _ => false,
            });
    let (call_ordinal, occurrence) = sites.next().filter(|_| sites.next().is_none())?;
    if occurrence.target_symbol(program) != target
        || match occurrence {
            crate::CallSite::Statement(_) => expression.is_valid(),
            crate::CallSite::Expression {
                expression: actual, ..
            } => actual != expression,
            crate::CallSite::TransitionNamed { .. } => true,
        }
    {
        return None;
    }
    Some(facts::InvocationOutputIdentity {
        state_symbol: state.symbol,
        statement_index,
        call_ordinal,
        parameter_symbol: parameter.symbol,
    })
}

#[allow(clippy::too_many_arguments)]
fn rebase_output_occurrences(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    site: super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    effects: &mut [SlotEffect],
) -> Option<()> {
    for effect in effects {
        for source in effect.sources.as_mut()? {
            if source.origin.source.output_identity().is_none() {
                continue;
            }
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == effect.root)?;
            let identity = output_identity(program, machine, site, target, expression, parameter)?;
            if identity.state_symbol != state.symbol {
                return None;
            }
            // A checked body supplies an opaque output on this normal route.
            // Caller instantiation owns the new descriptor occurrence; it may
            // not reuse an internal callee expression as a caller value epoch.
            let place = FrameSourcePlace::invocation_output(identity, effect.segments.clone());
            source.origin = FramePlaceOrigin {
                path: "invocation output".to_owned(),
                precision: FramePathPrecision::Exact,
                source: place.clone(),
            };
            source.carrier = place;
            source.carrier_backing = None;
        }
    }
    Some(())
}

fn path(program: &dyn crate::CheckingInput, source: &FrameSourcePlace) -> String {
    super::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    )
    .0
}
