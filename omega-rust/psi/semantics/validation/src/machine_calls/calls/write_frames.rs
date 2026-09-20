//! Conservative caller-visible write-frame inference.
//!
//! This module owns call-demand collection, internal and boundary call-frame
//! summaries, alias-origin propagation, and transition-cycle frame equations.
//! It produces complete caller-visible paths or fails closed as opaque; call
//! validity and type diagnostics remain owned by the parent module.
//!
//! This file owns the known-call summaries. `state_write_walk.rs` walks
//! state write prefixes, `alias_origins.rs` propagates stable alias origins,
//! `transparent_results.rs` follows transparent call results,
//! `parameter_relative_origins.rs` resolves parameter-relative origins and
//! `permuted_cycle_frames.rs` solves transition-cycle frame equations and
//! `wire_codecs.rs` frames synthesized wire codec calls from their argument
//! access; the remaining files carry the demand, alias, path and topology
//! vocabulary.

mod alias_bindings;
mod alias_origins;
mod assignment_targets;
mod boundary_calls;
mod call_targets;
mod call_trees;
mod caller_aliases;
mod demand;
mod inference;
mod isolated_initializers;
mod isolation;
mod local_aliases;
mod parameter_aliases;
mod parameter_relative_origins;
mod path_instantiation;
mod permuted_cycle_frames;
mod place_paths;
mod reference_origins;
mod reference_subjects;
mod result_origins;
mod state_paths;
mod state_write_walk;
mod stored_origins;
mod transition_equations;
mod transition_topology;
mod transparent_effects;
mod transparent_results;
mod type_capabilities;
mod type_instantiation;
mod value_expressions;
mod wire_codecs;

pub use alias_bindings::state_reference_parameter_binding_is_stable;
pub(crate) use boundary_calls::boundary_trait_signature;
pub(crate) use call_targets::free_machine_entry_state;
pub(crate) use call_targets::machine_state_by_symbol;
pub use caller_aliases::{AssignmentWriteTarget, LocalWriteOrigin};
pub(crate) use demand::statement_value_expression_roots;
pub use demand::{CallFrameResolver, frame_paths_overlap};
pub(crate) use transition_topology::named_state_transition_subgraph_is_acyclic;

use super::receiver_member_chain;
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use crate::machine_calls::calls::write_frames::state_write_walk::summarize_complete_state_written_paths;
use assignment_targets::expression_is_effectful_indexed_place;
use boundary_calls::known_boundary_call_written_paths_for_parts;
use call_trees::parameter_relative_expression_preserves_transparent_result;
use inference::FrameInference;
use isolation::type_is_caller_isolated_local;
use local_aliases::{expression_may_rebind_mutable_alias, rebase_local_alias_path};
use parameter_aliases::{
    ParameterRelativeFrameOrigin, expression_reborrows_transparent_alias_binding,
};
use path_instantiation::instantiate_written_path_with_origins;
use place_paths::{
    FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, append_place_suffix, coarse_place_path,
    frame_place_path, split_place_root,
};
use reference_origins::receiver_frame_origin;
use state_paths::normalize_state_relative_path;
use stored_origins::StoredLocalOrigins;
use transparent_effects::expression_is_effectful_for_transparent_result;
use type_capabilities::{type_may_carry_write, type_reference_is_reference};

#[cfg(test)]
thread_local! {
    static PREFIX_WALKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CYCLE_EQUATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Instantiate the conservative may-write set of a resolved internal call in
/// the caller's place namespace. `None` means the summary is not complete and
/// the caller must invalidate every flow fact. Internal acyclic calls and
/// state-transition graphs with complete expression frames compose;
/// implementation shapes this inference cannot summarize remain deliberately
/// opaque. Authored `stores` clauses are retired; precision grows through the
/// shared inferred complete-or-opaque frame instead.
fn known_call_written_paths_with_summaries(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    inference: &mut FrameInference,
    argument_origins: Option<&[Option<Vec<FramePlaceOrigin>>]>,
) -> Option<Vec<String>> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts_with_origins(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        None,
        program.statement_table.expression_handles(call.arguments),
        current_machine,
        machine_symbols,
        symbols,
        inference,
        argument_origins,
        complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    receiver_origin: Option<&FramePlaceOrigin>,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    known_call_written_paths_for_parts_with_origins(
        program,
        target_symbol,
        target,
        receiver_members,
        receiver_origin,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        inference,
        None,
        complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts_with_origins(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    receiver_origin: Option<&FramePlaceOrigin>,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    argument_origins: Option<&[Option<Vec<FramePlaceOrigin>>]>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    // A static machine parameter's selected target is a specialization input,
    // not an ordinary receiver binding. Until MP summaries instantiate that
    // binding explicitly, retain the sound all-facts invalidation.
    if program
        .machine_parameter_signature_in(current_machine, target_symbol)
        .is_some()
    {
        return None;
    }
    let exact_callee = machine_state_by_symbol(program, target_symbol);
    if receiver_origin
        .is_some_and(|origin| origin.precision == FramePathPrecision::CollectionCoarse)
        && exact_callee.is_none()
    {
        // A collection path is storage evidence, not a nominal receiver name.
        // It cannot select a same-named cached field or machine.
        return None;
    }
    let (callee_machine, callee_state) = exact_callee
        .or_else(|| {
            (receiver_members.is_empty()
                || matches!(receiver_members, [receiver] if receiver == "self"))
            .then(|| {
                machine_state_by_symbol(program, target_symbol)
                    .filter(|(machine, _)| machine.symbol != current_machine.symbol)
                    .or_else(|| {
                        machine_symbols
                            .state(target)
                            .map(|state| (current_machine, state))
                    })
                    .or_else(|| {
                        current_machine
                            .attached_data
                            .as_ref()
                            .and_then(|attached_data| {
                                symbols.attached_machine_state(
                                    program,
                                    attached_data.as_str(),
                                    target,
                                )
                            })
                    })
                    .or_else(|| free_machine_entry_state(program, symbols, target))
            })
            .flatten()
        })
        .or_else(|| {
            let receiver = receiver_members.last()?.as_str();
            let machine = machine_symbols
                .callable_field_type(receiver)
                .and_then(|type_name| symbols.machine(type_name))
                .or_else(|| symbols.machine(receiver))?;
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.name.as_str() == target)?;
            Some((machine, state))
        })?;

    if inference.active_states.contains(&callee_state.symbol) {
        return None;
    }
    summarize_resolved_call(
        program,
        current_machine,
        arguments,
        callee_machine,
        callee_state,
        receiver_members,
        receiver_origin,
        symbols,
        inference,
        argument_origins,
        complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn summarize_resolved_call(
    program: &TypedTrees,
    caller_machine: &Machine,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    receiver_members: &[String],
    receiver_origin: Option<&FramePlaceOrigin>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    argument_origins: Option<&[Option<Vec<FramePlaceOrigin>>]>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    let receiver_base = receiver_origin.cloned().or_else(|| {
        (!receiver_members.is_empty())
            .then(|| receiver_members.join("."))
            .or_else(|| {
                callee_machine
                    .attached_data
                    .as_ref()
                    .map(|_| "self".to_owned())
            })
            .map(|path| FramePlaceOrigin {
                path,
                precision: FramePathPrecision::Exact,
                source: Default::default(),
            })
    });
    let parameters = program.state_parameters(callee_state);
    let mut written = Vec::new();

    inference.active_states.push(callee_state.symbol);
    let relative_paths = summarize_complete_state_written_paths(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
        complete_state_summaries,
    );
    inference.active_states.pop();
    // Actual expressions run in the caller, not recursively inside this
    // callee body. A producer may call this same consumer in a finite tree;
    // only enclosing body guards belong to that actual's origin proof.
    for relative in relative_paths? {
        for instantiated in instantiate_written_path_with_origins(
            program,
            caller_machine,
            &relative,
            receiver_base.as_ref(),
            parameters,
            arguments,
            &[],
            symbols,
            inference,
            argument_origins,
        )? {
            if !written.contains(&instantiated) {
                written.push(instantiated);
            }
        }
    }

    Some(written)
}
