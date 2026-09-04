//! Conservative caller-visible write-frame inference.
//!
//! This module owns call-demand collection, internal and boundary call-frame
//! summaries, alias-origin propagation, and transition-cycle frame equations.
//! It produces complete caller-visible paths or fails closed as opaque; call
//! validity and type diagnostics remain owned by the parent module.

use super::receiver_member_chain;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall, TransitionTargetNode};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod alias_bindings;
mod assignment_targets;
mod boundary_calls;
mod call_targets;
mod call_trees;
mod demand;
mod isolated_initializers;
mod isolation;
mod local_aliases;
mod parameter_aliases;
mod path_instantiation;
mod place_paths;
mod state_paths;
mod transition_equations;
mod transition_topology;
mod transparent_effects;
mod type_capabilities;

use alias_bindings::{
    rebind_stable_local_mutable_alias_origin, stable_local_mutable_alias_rebinding_is_representable,
};
use assignment_targets::{
    assignment_target_type, expression_is_effectful_indexed_place,
    transparent_assignment_target_effect_is_structural,
};
use boundary_calls::known_boundary_call_written_paths_for_parts;
pub(crate) use boundary_calls::{boundary_trait_signature, known_boundary_call_written_paths};
use call_targets::discarded_primitive_internal_call_is_relationally_neutral;
pub(crate) use call_targets::free_machine_entry_state;
pub(super) use call_targets::machine_state_by_symbol;
use call_trees::{
    stable_alias_index_expression_preserves_origin,
    statement_call_argument_preserves_transparent_result,
};
pub use demand::{CallFrameResolver, frame_paths_overlap};
use demand::{collect_expression_call_written_paths, syntactic_call_written_paths};
pub(crate) use demand::{conservative_call_written_paths, statement_value_expression_roots};
use isolated_initializers::isolated_local_initializer_preserves_transparent_result;
use isolation::{
    struct_literal_field_is_primitive, struct_literal_field_type,
    struct_literal_matches_expected_type, struct_literal_type_is_caller_isolated,
    type_is_caller_isolated_local,
};
use local_aliases::{
    expression_may_rebind_mutable_alias, expression_reborrows_local_alias_binding,
    rebase_local_alias_path, stable_alias_place_origin,
};
use parameter_aliases::{
    ParameterRelativeFrameOrigin, expression_reborrows_transparent_alias_binding,
    parameter_relative_alias_position,
};
use path_instantiation::{instantiate_written_path, instantiate_written_path_with_origins};
use place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, coarse_place_path, frame_place_path,
    split_place_root,
};
use state_paths::{
    expression_forwards_exact_symbol, normalize_state_relative_path, push_visible_frame_path,
    relative_state_path_is_visible,
};
use transition_equations::{
    PermutedCycleFrameEquation, append_permuted_cycle_frame_edge, transition_state_reaches,
};
use transition_topology::{
    named_transition_preserves_state_namespace, named_transition_subgraph_is_acyclic,
    named_transition_target_state,
};
use transparent_effects::{
    call_is_transparent_mutable_slice_view, expression_is_effectful_for_transparent_result,
    frame_place_root_symbol,
};
use type_capabilities::{
    parameter_may_carry_write, type_may_carry_write, type_reference_is_reference,
};

/// Instantiate the conservative may-write set of a resolved internal call in
/// the caller's place namespace. `None` means the summary is not complete and
/// the caller must invalidate every flow fact. Internal acyclic calls and
/// state-transition graphs with complete expression frames compose;
/// implementation shapes this inference cannot summarize remain deliberately
/// opaque. Authored `stores` clauses are retired; precision grows through the
/// shared inferred complete-or-opaque frame instead.
pub(crate) fn known_call_written_paths(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    known_call_written_paths_with_summaries(
        program,
        call,
        current_machine,
        machine_symbols,
        symbols,
        &mut Vec::new(),
    )
}

fn known_call_written_paths_with_summaries(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
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
        program.statement_table.expression_handles(call.arguments),
        current_machine,
        machine_symbols,
        symbols,
        &mut Vec::new(),
        None,
        complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let mut complete_state_summaries = Vec::new();
    known_call_written_paths_for_parts_with_origins(
        program,
        target_symbol,
        target,
        receiver_members,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        active_states,
        None,
        &mut complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts_with_origins(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    known_call_written_paths_for_parts_with_origins_and_summaries(
        program,
        target_symbol,
        target,
        receiver_members,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        active_states,
        argument_origins,
        complete_state_summaries,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts_with_origins_and_summaries(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
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
    let (callee_machine, callee_state) = machine_state_by_symbol(program, target_symbol)
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

    if active_states.contains(&callee_state.symbol) {
        return None;
    }
    active_states.push(callee_state.symbol);
    let result = summarize_resolved_call(
        program,
        arguments,
        callee_machine,
        callee_state,
        receiver_members,
        symbols,
        active_states,
        argument_origins,
        complete_state_summaries,
    );
    active_states.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn summarize_resolved_call(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    receiver_members: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    let receiver_base = (!receiver_members.is_empty())
        .then(|| receiver_members.join("."))
        .or_else(|| {
            callee_machine
                .attached_data
                .as_ref()
                .map(|_| "self".to_owned())
        });
    let parameters = program.state_parameters(callee_state);
    let mut written = Vec::new();

    let relative_paths = summarize_state_written_paths(
        program,
        callee_machine,
        callee_state,
        symbols,
        active_states,
        complete_state_summaries,
    )
    .or_else(|| {
        summarize_state_written_paths_with_permuted_cycles(
            program,
            callee_machine,
            callee_state,
            symbols,
            active_states,
        )
    })?;
    for relative in relative_paths {
        if let Some(instantiated) = instantiate_written_path_with_origins(
            program,
            &relative,
            receiver_base.as_deref(),
            parameters,
            arguments,
            &[],
            symbols,
            active_states,
            argument_origins,
        )? && !written.contains(&instantiated)
        {
            written.push(instantiated);
        }
    }

    Some(written)
}

fn summarize_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    if let Some((_, paths)) = complete_state_summaries
        .iter()
        .find(|(symbol, _)| *symbol == state.symbol)
    {
        return Some(paths.clone());
    }
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut written = Vec::new();

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference) =>
            {
                stable_local_mutable_alias_origin(
                    program,
                    machine,
                    &machine_symbols,
                    active_states,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )
            }
            _ => None,
        };
        let representable_alias_rebinding = match statement {
            StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
                .is_some_and(|target| {
                    stable_local_mutable_alias_rebinding_is_representable(
                        program,
                        machine,
                        state,
                        &target,
                        assignment.value,
                        &local_alias_origins,
                        |aliases| {
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                &machine_symbols,
                                active_states,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                            )
                        },
                    )
                }),
            _ => false,
        };
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_local_alias_binding(program, expression, &local_alias_origins)
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
            {
                return None;
            }
            let mut expression_writes = Vec::new();
            collect_expression_call_written_paths(
                program,
                expression,
                machine,
                &machine_symbols,
                symbols,
                active_states,
                &mut expression_writes,
            )?;
            for relative in expression_writes {
                let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
        }
        match statement {
            StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                let direct_target = coarse_place_path(program, assignment.target);
                if let Some(relative) = direct_target.as_deref()
                    && rebind_stable_local_mutable_alias_origin(
                        program,
                        machine,
                        state,
                        relative,
                        assignment.value,
                        &mut local_alias_origins,
                        |aliases| {
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                &machine_symbols,
                                active_states,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                            )
                        },
                    )?
                {
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    &machine_symbols,
                    active_states,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
            StatementNode::Call(nested_call) => {
                let nested_receiver_members = program
                    .statement_table
                    .name_path_members(nested_call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let arguments = program
                    .statement_table
                    .expression_handles(nested_call.arguments);
                let argument_origins = arguments
                    .iter()
                    .map(|argument| {
                        stable_alias_initializer_origin(
                            program,
                            machine,
                            &machine_symbols,
                            active_states,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins_and_summaries(
                    program,
                    nested_call.target_symbol,
                    nested_call.target.as_str(),
                    &nested_receiver_members,
                    arguments,
                    machine,
                    &machine_symbols,
                    symbols,
                    active_states,
                    Some(&argument_origins),
                    complete_state_summaries,
                )
                .or_else(|| {
                    (!arguments
                        .iter()
                        .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
                    .then(|| {
                        known_boundary_call_written_paths_for_parts(
                            program,
                            &machine_symbols,
                            symbols,
                            &nested_receiver_members,
                            nested_call.target.as_str(),
                            arguments,
                        )
                    })
                    .flatten()
                })
                .or_else(|| {
                    syntactic_call_written_paths(program, &nested_receiver_members, arguments)
                })?;
                for relative in nested_writes {
                    let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                    if relative_state_path_is_visible(&relative, parameters, &locals)?
                        && !written.contains(&relative)
                    {
                        written.push(relative);
                    }
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    if !local_alias_origins.is_empty()
                        && target.is_valid()
                        && matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Named { .. }
                        )
                        && !named_transition_subgraph_is_acyclic(program, machine, state, target)
                    {
                        // Alias origins compose positionally through acyclic
                        // named graphs. A reachable named SCC needs equation-
                        // level alias substitution, so keep it opaque here.
                        return None;
                    }
                    for relative in summarize_transition_target_written_paths(
                        program,
                        machine,
                        state,
                        target,
                        symbols,
                        active_states,
                        complete_state_summaries,
                        &locals,
                    )? {
                        let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                        if relative_state_path_is_visible(&relative, parameters, &locals)?
                            && !written.contains(&relative)
                        {
                            written.push(relative);
                        }
                    }
                }
            }
            StatementNode::Expression(_) => {}
            StatementNode::LocalData(local) => {
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                {
                    let origin = declared_local_alias_origin?;
                    local_alias_origins.push((local.name.as_str().to_owned(), origin));
                }
                if type_is_caller_isolated_local(program, local.type_reference) {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    complete_state_summaries.push((state.symbol, written.clone()));
    Some(written)
}

/// Recover the caller-visible origin of the deliberately narrow stable local-
/// alias shapes handled by the ordinary frame summarizer. A reborrow through
/// an already-known alias composes an exact member suffix onto that alias's
/// origin. An indexed reborrow is represented by its whole collection; once
/// coarse, later member suffixes may never narrow that collection again.
/// Caller-isolated locals are also stable origins, but remain local-only and
/// therefore disappear from the published caller frame. A structurally
/// transparent value call preserves such an origin just as it preserves a
/// caller-visible parameter origin. The compiler-owned `as_mut_slice` view
/// preserves its receiver's storage origin. A validated mutable recast is
/// address identity, so an effect-free recast source preserves the same origin.
/// Direct parameter-relative member and effect-free indexed projections compose
/// the same exact/coarse path algebra. Other computed results stay opaque.
fn stable_local_mutable_alias_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    local: &psi_typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<FramePlaceOrigin> {
    let TypeReferenceNode::Reference { access, .. } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    if !access.is_exclusive() {
        return None;
    }
    stable_alias_initializer_origin(
        program,
        current_machine,
        machine_symbols,
        active_states,
        local.initial_value,
        parameters,
        isolated_local_roots,
        aliases,
        symbols,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_initializer_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            active_states,
            inner.target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_initializer_origin(
                    program,
                    current_machine,
                    machine_symbols,
                    active_states,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                );
            }
            transparent_call_result_origin(program, call, symbols, |actual| {
                stable_alias_initializer_origin(
                    program,
                    current_machine,
                    machine_symbols,
                    active_states,
                    actual,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                )
            })
        }
        ExpressionNode::Indexed(indexed)
            if expression_is_effectful_for_transparent_result(program, indexed.index) =>
        {
            if !stable_alias_index_expression_preserves_origin(
                program,
                current_machine,
                indexed.index,
                machine_symbols,
                symbols,
                active_states,
                parameters,
                aliases,
            ) {
                return None;
            }
            let mut collection = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                active_states,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                active_states,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => stable_alias_expression_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => stable_alias_expression_origin(
            program,
            inner.target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_expression_origin(
                    program,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                );
            }
            transparent_call_result_origin(program, call, symbols, |actual| {
                stable_alias_expression_origin(
                    program,
                    actual,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                )
            })
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            stable_alias_expression_origin(
                program,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut collection = stable_alias_expression_origin(
                program,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_expression_origin(
                program,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => stable_alias_place_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            allow_isolated_local,
        ),
    }
}

/// Resolve an assignment target using the established direct-place behavior,
/// plus the structural origin algebra shared by stable aliases. This
/// admits a validated effectful index through a stable alias or transparent
/// helper result while preserving the rebinding and opacity fences.
#[allow(clippy::too_many_arguments)]
fn stable_assignment_target_path(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    target: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<String> {
    if let Some(relative) = coarse_place_path(program, target) {
        return Some(rebase_local_alias_path(&relative, aliases));
    }
    Some(
        stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            active_states,
            target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            true,
        )?
        .path,
    )
}

/// Recover one deliberately structural value-call relation. The helper may be
/// free or attached, but must be acyclic at the result surface, return `&mut`,
/// and have one terminal result expression rooted in one mutable-reference
/// parameter. A prefix may contain caller-isolated scratch locals and local
/// mutable-reference bindings that forward direct places from that parameter, an
/// earlier such local, or another structurally transparent helper. Value-shaped
/// assignments with effect-free right-hand sides may write through those
/// places, scratch locals, validated mutable recast aliases with effect-free
/// sources, or exact transparent call-produced targets without changing their
/// origins; the ordinary frame summary still publishes caller-visible writes.
/// Effect-free discarded expressions are also neutral.
/// A direct stable alias rebind updates only that local; prior reborrows retain
/// their established origins. Explicit arguments and an attached helper's
/// actual receiver both supply exact caller origins. This is body evidence, not
/// lifetime elision: a reference-bearing scratch local, opaque computed rebind,
/// discarded/statement call, recursive helper relation, named-state route, or
/// alternate result fails closed.
fn transparent_call_result_origin(
    program: &TypedTrees,
    call: &TableCallExpression,
    symbols: &TopLevelSymbols<'_>,
    resolve_actual_origin: impl FnOnce(ExpressionHandle) -> Option<FramePlaceOrigin>,
) -> Option<FramePlaceOrigin> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }

    let result_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        &mut Vec::new(),
    )?;
    let parameters = program.state_parameters(callee_state);
    let result_parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
    let (_, result_suffix) = split_place_root(&result_origin.place.path);
    let actual = if result_parameter.is_self {
        if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
            return None;
        }
        call.receiver
    } else {
        let (argument_index, _) = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
        *program
            .expression_table
            .expression_handles(call.arguments)
            .get(argument_index)?
    };
    let argument_origin = resolve_actual_origin(actual)?;
    Some(match argument_origin.precision {
        FramePathPrecision::Exact => FramePlaceOrigin {
            path: append_place_suffix(&argument_origin.path, result_suffix),
            precision: result_origin.place.precision,
        },
        FramePathPrecision::CollectionCoarse => argument_origin,
    })
}

/// Recover a transparent returned-place origin without imposing a caller
/// namespace. This is used while instantiating a callee frame through a nested
/// statement-call argument: direct places keep their existing spelling, while
/// the compiler-owned `as_mut_slice` view preserves its receiver and a
/// structurally transparent helper selects and composes one of its actual
/// mutable-reference origins. Opaque or recursive helpers fail closed.
fn transparent_place_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            transparent_place_expression_origin(program, inner.target, symbols, active_states)
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut origin = transparent_place_expression_origin(
                program,
                indexed.collection,
                symbols,
                active_states,
            )?;
            origin.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let origin = transparent_place_expression_origin(
                program,
                member.receiver,
                symbols,
                active_states,
            )?;
            Some(match origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", origin.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => origin,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return transparent_place_expression_origin(
                    program,
                    call.receiver,
                    symbols,
                    active_states,
                );
            }
            let (callee_machine, callee_state) =
                machine_state_by_symbol(program, call.target_symbol).or_else(|| {
                    (!call.receiver.is_valid())
                        .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                        .flatten()
                })?;
            if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
                return None;
            }
            let result_origin = transparent_callee_result_origin(
                program,
                callee_machine,
                callee_state,
                symbols,
                active_states,
            )?;
            let parameters = program.state_parameters(callee_state);
            let result_parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
            let actual = if result_parameter.is_self {
                if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
                    return None;
                }
                call.receiver
            } else {
                let (argument_index, _) = parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
                *program
                    .expression_table
                    .expression_handles(call.arguments)
                    .get(argument_index)?
            };
            let actual_origin =
                transparent_place_expression_origin(program, actual, symbols, active_states)?;
            let (_, suffix) = split_place_root(&result_origin.place.path);
            Some(match actual_origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: append_place_suffix(&actual_origin.path, suffix),
                    precision: result_origin.place.precision,
                },
                FramePathPrecision::CollectionCoarse => actual_origin,
            })
        }
        _ => frame_place_path(program, expression),
    }
}

fn transparent_callee_result_origin(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    if active_states.contains(&callee_state.symbol)
        || !matches!(
            program
                .type_reference_table
                .type_reference(callee_state.return_type),
            TypeReferenceNode::Reference { access, .. }
                if access.is_exclusive()
        )
    {
        return None;
    }
    active_states.push(callee_state.symbol);
    let result = (|| {
        let statements = program
            .statement_table
            .statements(callee_state.statement_nodes);
        let (StatementNode::Expression(result), prefix) = statements.split_last()? else {
            return None;
        };

        let parameters = program.state_parameters(callee_state);
        let mut local_aliases = Vec::new();
        let mut isolated_local_roots = Vec::new();
        for statement in prefix {
            match statement {
                StatementNode::LocalData(local) => {
                    if type_is_caller_isolated_local(program, local.type_reference) {
                        if !isolated_local_initializer_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            &isolated_local_roots,
                            |machine_symbols, written| {
                                collect_expression_call_written_paths(
                                    program,
                                    local.initial_value,
                                    callee_machine,
                                    machine_symbols,
                                    symbols,
                                    active_states,
                                    written,
                                )
                            },
                        ) {
                            return None;
                        }
                        isolated_local_roots.push(local.name.as_str().to_owned());
                        continue;
                    }
                    let TypeReferenceNode::Reference { access, .. } = program
                        .type_reference_table
                        .type_reference(local.type_reference)
                    else {
                        return None;
                    };
                    if !access.is_exclusive() {
                        return None;
                    }
                    let origin = parameter_relative_place_origin(
                        program,
                        callee_machine,
                        local.initial_value,
                        parameters,
                        &local_aliases,
                        symbols,
                        active_states,
                    )?;
                    local_aliases.push((local.name.as_str().to_owned(), local.symbol, origin));
                }
                StatementNode::Assignment(assignment) => {
                    if expression_is_effectful_for_transparent_result(program, assignment.target)
                        && (!transparent_assignment_target_effect_is_structural(
                            program,
                            assignment.target,
                        ) || parameter_relative_place_origin(
                            program,
                            callee_machine,
                            assignment.target,
                            parameters,
                            &local_aliases,
                            symbols,
                            active_states,
                        )
                        .is_none())
                    {
                        return None;
                    }
                    if value_expression_assignment_preserves_transparent_result(
                        program,
                        callee_machine,
                        assignment.value,
                        assignment_target_type(
                            program,
                            callee_machine,
                            callee_state,
                            assignment.target,
                        ),
                        symbols,
                        active_states,
                        parameters,
                        &local_aliases,
                    ) {
                        // A non-reference call result can change only the
                        // target's value, not an established alias origin.
                    } else if expression_may_rebind_mutable_alias(
                        program,
                        callee_machine,
                        callee_state,
                        assignment.value,
                    ) {
                        let position = parameter_relative_alias_position(
                            program,
                            assignment.target,
                            &local_aliases,
                        )?;
                        let replacement = parameter_relative_place_origin(
                            program,
                            callee_machine,
                            assignment.value,
                            parameters,
                            &local_aliases,
                            symbols,
                            active_states,
                        )?;
                        local_aliases[position].2 = replacement;
                    } else if expression_is_effectful_for_transparent_result(
                        program,
                        assignment.value,
                    ) {
                        return None;
                    }
                }
                StatementNode::Expression(expression)
                    if !expression_is_effectful_for_transparent_result(program, *expression) => {}
                StatementNode::Call(call)
                    if statement_call_preserves_transparent_result(
                        program,
                        callee_machine,
                        call,
                        symbols,
                        active_states,
                        parameters,
                        &local_aliases,
                    ) => {}
                _ => return None,
            }
        }
        parameter_relative_place_origin(
            program,
            callee_machine,
            *result,
            parameters,
            &local_aliases,
            symbols,
            active_states,
        )
    })();
    active_states.pop();
    result
}

/// One direct Unit statement call is neutral to a returned-place relation when
/// its inferred frame is complete and no argument exposes a mutable-reference
/// binding for rebinding. The same applies to an explicitly discarded concrete
/// primitive result from a nongeneric checked-body call: the value cannot carry
/// an alias, while the call's complete frame still publishes its writes. Writes
/// through references passed by value may change their contents, but cannot
/// redirect the established origin. One direct value-call argument is admitted
/// under the same complete-frame rule. Other discarded results, deeper computed
/// arguments, binding reborrows, and opaque frames remain fences.
fn statement_call_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCall,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    if call.discards_result
        && !discarded_primitive_internal_call_is_relationally_neutral(program, call, symbols)
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Every sibling must independently preserve the returned-place origin.
    if arguments.iter().any(|argument| {
        !statement_call_argument_preserves_transparent_result(
            program,
            current_machine,
            *argument,
            &machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
        )
    }) {
        return false;
    }

    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    let argument_origins = arguments
        .iter()
        .map(|argument| {
            parameter_relative_place_origin(
                program,
                current_machine,
                *argument,
                parameters,
                aliases,
                symbols,
                active_states,
            )
            .map(|origin| origin.place)
        })
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts_with_origins(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        arguments,
        current_machine,
        &machine_symbols,
        symbols,
        active_states,
        Some(&argument_origins),
        &mut Vec::new(),
    )
    .or_else(|| {
        (!arguments
            .iter()
            .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
        .then(|| {
            known_boundary_call_written_paths_for_parts(
                program,
                &machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                arguments,
            )
        })
        .flatten()
    })
    .is_some()
}

/// A complete finite direct-call tree may supply an assignment value without
/// perturbing a separately returned place only when its root result is proven
/// non-reference. A direct primitive scalar value may wrap complete
/// caller-isolated call producers in up to thirty-three unary, binary, primitive-cast,
/// member-projection, or indexing shells. One
/// primitive-only record, selected-case, or fixed-array literal may
/// independently contain such a tree in each direct field/element, and up to
/// two nested aggregates of those concrete kinds may do the same under their
/// existing two-shell computation budget. Projected aggregate literals retain
/// their narrower depth-two rail. Reference-bearing or generic literals, wider
/// aggregate or scalar-computation depth, and unknown return types fail closed.
const TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_AGGREGATE_DEPTH: usize = 3;
const TRANSPARENT_ASSIGNMENT_VALUE_PROJECTED_AGGREGATE_DEPTH: usize = 2;
const TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH: usize = 33;
const TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH: usize = 2;

fn value_expression_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    assignment_target_type: Option<TypeReferenceHandle>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
            program,
            current_machine,
            expression,
            symbols,
            active_states,
            parameters,
            aliases,
        ),
        ExpressionNode::StructLiteral(_) => {
            aggregate_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_AGGREGATE_DEPTH,
            )
        }
        ExpressionNode::ArrayLiteral(_) => assignment_target_type.is_some_and(|target_type| {
            array_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                expression,
                target_type,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_AGGREGATE_DEPTH,
                TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
            )
        }),
        ExpressionNode::Binary(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Unary(_)
            if assignment_target_type.is_some_and(|target_type| {
                program.primitive_type_reference(target_type).is_some()
            }) =>
        {
            primitive_computed_assignment_value_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH,
            )
        }
        ExpressionNode::Cast(cast)
            if assignment_target_type.is_some_and(|target_type| {
                program.primitive_type_reference(target_type).is_some()
            }) && program.primitive_type_reference(cast.target_type).is_some() =>
        {
            primitive_computed_assignment_value_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH,
            )
        }
        _ => false,
    }
}

/// Apply the concrete aggregate-value rail to one fixed-array literal. The
/// assignment target supplies the exact contextual element type that an array
/// literal does not carry itself. Only literal-length, caller-isolated arrays
/// participate; every effectful element independently obeys the ordinary
/// complete direct-call rule, primitive elements may use the carried scalar
/// computation budget, and one nested fixed-array or concrete aggregate
/// literal consumes the second aggregate level.
#[allow(clippy::too_many_arguments)]
fn array_value_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
    remaining_computed_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 || !type_is_caller_isolated_local(program, expected_type) {
        return false;
    }
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
    else {
        return false;
    };
    let Some(expected_type) = crate::places::unwrapped_type_reference(program, expected_type)
    else {
        return false;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length: psi_typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(expected_type)
    else {
        return false;
    };
    let elements = program.expression_table.expression_handles(*elements);
    if *length != elements.len() {
        return false;
    }
    let Some(element_type) = crate::places::unwrapped_type_reference(program, *element_type) else {
        return false;
    };
    elements.iter().all(|element| {
        if !expression_is_effectful_for_transparent_result(program, *element) {
            return true;
        }
        match program.expression_table.expression(*element) {
            ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                program,
                current_machine,
                *element,
                symbols,
                active_states,
                parameters,
                aliases,
            ),
            ExpressionNode::ArrayLiteral(_) => array_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                *element,
                element_type,
                symbols,
                active_states,
                parameters,
                aliases,
                remaining_aggregate_depth - 1,
                remaining_computed_depth,
            ),
            ExpressionNode::StructLiteral(literal)
                if struct_literal_matches_expected_type(program, literal, element_type) =>
            {
                aggregate_value_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_aggregate_depth - 1,
                )
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Unary(_)
                if program.primitive_type_reference(element_type).is_some() =>
            {
                primitive_computed_value_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_computed_depth,
                    false,
                )
            }
            _ => false,
        }
    })
}

/// Apply the settled primitive aggregate-field computation algebra directly to
/// a scalar assignment value. The primitive assignment target supplies the
/// typed result fact; every effectful call producer must return a caller-
/// isolated value, so generic/reference carriers remain fenced while concrete
/// records and fixed arrays returned by calls can feed member/index shells. A
/// direct member projection may additionally select from one concrete literal
/// whose effectful fields are finite direct-call trees, and may itself sit
/// below one further computation shell. Concrete literals retain that separate
/// two-shell frontier instead of inheriting the thirty-three direct-scalar shells.
/// Aggregate fields remain at two shells as well, and a thirty-fourth direct-scalar
/// computation shell fails closed.
#[allow(clippy::too_many_arguments)]
fn primitive_computed_assignment_value_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_computed_depth: usize,
) -> bool {
    primitive_computed_value_preserves_transparent_result(
        program,
        current_machine,
        expression,
        symbols,
        active_states,
        parameters,
        aliases,
        remaining_computed_depth,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn caller_isolated_value_call_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some((_, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol).or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    type_is_caller_isolated_local(program, callee_state.return_type)
        && value_call_assignment_preserves_transparent_result(
            program,
            current_machine,
            expression,
            symbols,
            active_states,
            parameters,
            aliases,
        )
}

#[allow(clippy::too_many_arguments)]
fn aggregate_value_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 {
        return false;
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return false;
    };
    if !struct_literal_type_is_caller_isolated(program, literal) {
        return false;
    }
    program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .all(|field| {
            if !expression_is_effectful_for_transparent_result(program, field.value) {
                return true;
            }
            match program.expression_table.expression(field.value) {
                ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    field.value,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                ),
                ExpressionNode::StructLiteral(_) => {
                    aggregate_value_assignment_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        remaining_aggregate_depth - 1,
                    )
                }
                ExpressionNode::ArrayLiteral(_) => {
                    struct_literal_field_type(program, literal, field.name.as_str()).is_some_and(
                        |field_type| {
                            array_value_assignment_preserves_transparent_result(
                                program,
                                current_machine,
                                field.value,
                                field_type,
                                symbols,
                                active_states,
                                parameters,
                                aliases,
                                remaining_aggregate_depth - 1,
                                TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                            )
                        },
                    )
                }
                ExpressionNode::Binary(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Cast(cast)
                    if program.primitive_type_reference(cast.target_type).is_some()
                        && struct_literal_field_is_primitive(
                            program,
                            literal,
                            field.name.as_str(),
                        ) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Unary(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                        false,
                    )
                }
                _ => false,
            }
        })
}

#[allow(clippy::too_many_arguments)]
fn primitive_computed_value_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_computed_depth: usize,
    require_caller_isolated_call_result: bool,
) -> bool {
    if remaining_computed_depth == 0 {
        return false;
    }
    let direct_concrete_literal_member = require_caller_isolated_call_result
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        );
    let direct_array_literal_index = require_caller_isolated_call_result
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Indexed(_)
        );
    let operands = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => [Some(binary.left), Some(binary.right)],
        ExpressionNode::Cast(cast)
            if program.primitive_type_reference(cast.target_type).is_some() =>
        {
            [Some(cast.value), None]
        }
        ExpressionNode::Indexed(indexed) => [Some(indexed.collection), Some(indexed.index)],
        ExpressionNode::Member(member) => [Some(member.receiver), None],
        ExpressionNode::Unary(unary) => [Some(unary.operand), None],
        _ => return false,
    };
    operands.into_iter().flatten().all(|operand| {
        if !expression_is_effectful_for_transparent_result(program, operand) {
            return true;
        }
        match program.expression_table.expression(operand) {
            ExpressionNode::Call(_) if require_caller_isolated_call_result => {
                caller_isolated_value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                )
            }
            ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                program,
                current_machine,
                operand,
                symbols,
                active_states,
                parameters,
                aliases,
            ),
            ExpressionNode::StructLiteral(_)
                if direct_concrete_literal_member
                    && remaining_computed_depth
                        > TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH
                            - TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH =>
            {
                concrete_literal_member_operand_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    TRANSPARENT_ASSIGNMENT_VALUE_PROJECTED_AGGREGATE_DEPTH,
                    remaining_computed_depth.saturating_sub(1).saturating_sub(
                        TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH
                            - TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                    ),
                )
            }
            ExpressionNode::ArrayLiteral(_)
                if direct_array_literal_index
                    && remaining_computed_depth
                        > TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH
                            - TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH =>
            {
                concrete_array_literal_index_operand_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    TRANSPARENT_ASSIGNMENT_VALUE_PROJECTED_AGGREGATE_DEPTH,
                    remaining_computed_depth.saturating_sub(1).saturating_sub(
                        TRANSPARENT_ASSIGNMENT_VALUE_DIRECT_COMPUTED_DEPTH
                            - TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_COMPUTED_DEPTH,
                    ),
                )
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Unary(_)
                if remaining_computed_depth > 1 =>
            {
                primitive_computed_value_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_computed_depth - 1,
                    require_caller_isolated_call_result,
                )
            }
            _ => false,
        }
    })
}

/// Admit one fixed-array literal directly below a primitive index projection.
/// Typing has already established one primitive result through the complete
/// index chain. Every eagerly evaluated element publishes its complete call
/// frame; primitive computation shells share the same depth budget consumed by
/// the index projection. A nested array literal may consume the existing
/// aggregate-depth-two rail without resetting either budget. Records and other
/// aggregates remain outside this cohort because an array element carries no
/// independent contextual nominal type for validating them.
#[allow(clippy::too_many_arguments)]
fn concrete_array_literal_index_operand_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
    remaining_computed_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 {
        return false;
    }
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
    else {
        return false;
    };
    program
        .expression_table
        .expression_handles(*elements)
        .iter()
        .all(|element| {
            if !expression_is_effectful_for_transparent_result(program, *element) {
                return true;
            }
            match program.expression_table.expression(*element) {
                ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                ),
                ExpressionNode::ArrayLiteral(_) if remaining_aggregate_depth > 1 => {
                    concrete_array_literal_index_operand_preserves_transparent_result(
                        program,
                        current_machine,
                        *element,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        remaining_aggregate_depth - 1,
                        remaining_computed_depth,
                    )
                }
                ExpressionNode::Binary(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Unary(_)
                    if remaining_computed_depth > 0 =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        *element,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        remaining_computed_depth,
                        false,
                    )
                }
                _ => false,
            }
        })
}

/// Admit one named concrete literal directly below a primitive member
/// projection, including when that projection sits below one outer computation
/// shell. The literal's type makes the aggregate shape explicit, unlike a
/// projected array literal whose contextual fixed-array type is unavailable
/// after the scalar projection. Effectful primitive fields may use only the
/// computation budget left after the member projection itself. Nested
/// aggregates consume the same explicit aggregate-depth budget as ordinary
/// aggregate assignments while retaining that reduced computation budget.
/// Any shell or nested literal that would reset or exceed either budget stays
/// outside this narrow cohort.
#[allow(clippy::too_many_arguments)]
fn concrete_literal_member_operand_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
    remaining_computed_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 {
        return false;
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return false;
    };
    struct_literal_type_is_caller_isolated(program, literal)
        && program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| {
                if !expression_is_effectful_for_transparent_result(program, field.value) {
                    return true;
                }
                match program.expression_table.expression(field.value) {
                    ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                    ),
                    ExpressionNode::StructLiteral(_) if remaining_aggregate_depth > 1 => {
                        concrete_literal_member_operand_preserves_transparent_result(
                            program,
                            current_machine,
                            field.value,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                            remaining_aggregate_depth - 1,
                            remaining_computed_depth,
                        )
                    }
                    ExpressionNode::ArrayLiteral(_) if remaining_aggregate_depth > 1 => {
                        struct_literal_field_type(program, literal, field.name.as_str())
                            .is_some_and(|field_type| {
                                array_value_assignment_preserves_transparent_result(
                                    program,
                                    current_machine,
                                    field.value,
                                    field_type,
                                    symbols,
                                    active_states,
                                    parameters,
                                    aliases,
                                    remaining_aggregate_depth - 1,
                                    remaining_computed_depth,
                                )
                            })
                    }
                    ExpressionNode::Binary(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Unary(_)
                        if remaining_computed_depth > 0
                            && struct_literal_field_is_primitive(
                                program,
                                literal,
                                field.name.as_str(),
                            ) =>
                    {
                        primitive_computed_value_preserves_transparent_result(
                            program,
                            current_machine,
                            field.value,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                            remaining_computed_depth,
                            false,
                        )
                    }
                    _ => false,
                }
            })
}

fn value_call_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some((_, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol).or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    if !callee_state.return_type.is_valid()
        || type_reference_is_reference(program, callee_state.return_type)
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    diagnostics.is_empty()
        && statement_call_argument_preserves_transparent_result(
            program,
            current_machine,
            expression,
            &machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
        )
}

fn parameter_relative_place_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => parameter_relative_place_origin(
            program,
            current_machine,
            inner.target,
            parameters,
            aliases,
            symbols,
            active_states,
        ),
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                let mut diagnostics = Vec::new();
                let machine_symbols =
                    MachineSymbols::build(program, current_machine, &mut diagnostics);
                if !diagnostics.is_empty()
                    || !statement_call_argument_preserves_transparent_result(
                        program,
                        current_machine,
                        indexed.index,
                        &machine_symbols,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                    )
                {
                    return None;
                }
            }
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                indexed.collection,
                parameters,
                aliases,
                symbols,
                active_states,
            )?;
            origin.place.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                member.receiver,
                parameters,
                aliases,
                symbols,
                active_states,
            )?;
            if origin.place.precision == FramePathPrecision::Exact {
                origin.place.path = format!("{}.{}", origin.place.path, member.member.as_str());
            }
            Some(origin)
        }
        ExpressionNode::Name(_) => {
            let place = frame_place_path(program, expression)?;
            let root_symbol = frame_place_root_symbol(program, expression);
            let (root, suffix) = split_place_root(&place.path);
            if let Some(parameter) = parameters.iter().find(|parameter| {
                (root_symbol == Some(parameter.symbol) || (parameter.is_self && root == "self"))
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference { access, .. }
                            if access.is_exclusive()
                    )
            }) {
                return Some(ParameterRelativeFrameOrigin {
                    place,
                    parameter_symbol: parameter.symbol,
                });
            }
            let parent = aliases.iter().find_map(|(name, symbol, origin)| {
                let exact_symbol = root_symbol
                    .is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
                let unresolved_name =
                    root_symbol.is_none_or(|root| !root.is_valid()) && name == root;
                (exact_symbol || unresolved_name).then_some(origin)
            })?;
            Some(ParameterRelativeFrameOrigin {
                place: match parent.place.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&parent.place.path, suffix),
                        precision: place.precision,
                    },
                    FramePathPrecision::CollectionCoarse => parent.place.clone(),
                },
                parameter_symbol: parent.parameter_symbol,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return parameter_relative_place_origin(
                    program,
                    current_machine,
                    call.receiver,
                    parameters,
                    aliases,
                    symbols,
                    active_states,
                );
            }
            parameter_relative_call_result_origin(
                program,
                current_machine,
                call,
                parameters,
                aliases,
                symbols,
                active_states,
            )
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            parameter_relative_place_origin(
                program,
                current_machine,
                cast.value,
                parameters,
                aliases,
                symbols,
                active_states,
            )
        }
        _ => None,
    }
}

fn parameter_relative_call_result_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
    caller_parameters: &[StateParameter],
    caller_aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }
    let callee_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        active_states,
    )?;
    let callee_parameters = program.state_parameters(callee_state);
    let callee_parameter = callee_parameters
        .iter()
        .find(|parameter| parameter.symbol == callee_origin.parameter_symbol)?;
    let actual = if callee_parameter.is_self {
        if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
            return None;
        }
        call.receiver
    } else {
        let (argument_index, _) = callee_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == callee_parameter.symbol)?;
        *program
            .expression_table
            .expression_handles(call.arguments)
            .get(argument_index)?
    };
    let actual_origin = parameter_relative_place_origin(
        program,
        current_machine,
        actual,
        caller_parameters,
        caller_aliases,
        symbols,
        active_states,
    )?;
    let (_, suffix) = split_place_root(&callee_origin.place.path);
    Some(match actual_origin.place.precision {
        FramePathPrecision::Exact => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                path: append_place_suffix(&actual_origin.place.path, suffix),
                precision: callee_origin.place.precision,
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
        FramePathPrecision::CollectionCoarse => actual_origin,
    })
}

/// Recover an exact finite frame for transition SCCs whose write-capable state
/// parameters are only permuted around each cycle.
///
/// The ordinary recursive summarizer above deliberately fails closed when it
/// reaches a named cycle that redirects an exclusive parameter. A permutation
/// is nevertheless finite: repeated traversal can only move an already-known
/// path among the SCC's positional roots. This fallback solves the reachable
/// state equations to a fixed point after proving that every cyclic edge is an
/// exact bijection over those write-capable roots. Structurally transparent
/// returned places preserve the root they forward. Projections, opaque helper
/// results, duplication, omission, and computed rebinding stay opaque;
/// otherwise suffix growth could make the path set unbounded or alias two
/// semantic roots.
fn summarize_state_written_paths_with_permuted_cycles<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'program>,
    outer_active_states: &[SymbolHandle],
) -> Option<Vec<String>> {
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return None;
    }

    let mut equations = Vec::<PermutedCycleFrameEquation<'program>>::new();
    let mut pending = vec![entry.symbol];
    while let Some(symbol) = pending.pop() {
        if equations
            .iter()
            .any(|equation| equation.state.symbol == symbol)
        {
            continue;
        }
        let state = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == symbol)?;
        let equation = build_permuted_cycle_frame_equation(
            program,
            machine,
            state,
            symbols,
            &machine_symbols,
            outer_active_states,
        )?;
        pending.extend(equation.edges.iter().map(|edge| edge.target));
        equations.push(equation);
    }

    let mut has_transition_cycle = false;
    for equation in &equations {
        for edge in &equation.edges {
            if transition_state_reaches(&equations, edge.target, equation.state.symbol) {
                has_transition_cycle = true;
                let target = equations
                    .iter()
                    .find(|candidate| candidate.state.symbol == edge.target)?
                    .state;
                let mut active_states = outer_active_states.to_vec();
                if !transition_is_exact_write_parameter_permutation(
                    program,
                    equation.state,
                    target,
                    &edge.arguments,
                    &equation.local_alias_origins,
                    symbols,
                    &mut active_states,
                ) {
                    return None;
                }
            }
        }
    }
    if !has_transition_cycle {
        return None;
    }

    let mut summaries = equations
        .iter()
        .map(|equation| (equation.state.symbol, equation.direct_writes.clone()))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for equation in &equations {
            for edge in &equation.edges {
                let target = equations
                    .iter()
                    .find(|candidate| candidate.state.symbol == edge.target)?;
                let target_writes = summaries
                    .iter()
                    .find(|(symbol, _)| *symbol == edge.target)?
                    .1
                    .clone();
                for relative in target_writes {
                    let Some(instantiated) = instantiate_written_path(
                        program,
                        &relative,
                        Some("self"),
                        program.state_parameters(target.state),
                        &edge.arguments,
                        &equation.locals,
                        symbols,
                        &mut outer_active_states.to_vec(),
                    )?
                    else {
                        continue;
                    };
                    let instantiated =
                        rebase_local_alias_path(&instantiated, &equation.local_alias_origins);
                    if !relative_state_path_is_visible(
                        &instantiated,
                        program.state_parameters(equation.state),
                        &equation.locals,
                    )? {
                        continue;
                    }
                    let source_writes = summaries
                        .iter_mut()
                        .find(|(symbol, _)| *symbol == equation.state.symbol)?;
                    if !source_writes.1.contains(&instantiated) {
                        source_writes.1.push(instantiated);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    summaries
        .into_iter()
        .find_map(|(symbol, writes)| (symbol == entry.symbol).then_some(writes))
}

fn build_permuted_cycle_frame_equation<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    symbols: &TopLevelSymbols<'program>,
    machine_symbols: &MachineSymbols<'program>,
    outer_active_states: &[SymbolHandle],
) -> Option<PermutedCycleFrameEquation<'program>> {
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut direct_writes = Vec::new();
    let mut edges = Vec::new();
    let mut active_states = outer_active_states.to_vec();
    if !active_states.contains(&state.symbol) {
        active_states.push(state.symbol);
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference) =>
            {
                stable_local_mutable_alias_origin(
                    program,
                    machine,
                    machine_symbols,
                    &mut active_states,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )
            }
            _ => None,
        };
        let representable_alias_rebinding = match statement {
            StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
                .is_some_and(|target| {
                    stable_local_mutable_alias_rebinding_is_representable(
                        program,
                        machine,
                        state,
                        &target,
                        assignment.value,
                        &local_alias_origins,
                        |aliases| {
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                machine_symbols,
                                &mut active_states,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                            )
                        },
                    )
                }),
            _ => false,
        };
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_local_alias_binding(program, expression, &local_alias_origins)
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
            {
                return None;
            }
            let mut expression_writes = Vec::new();
            collect_expression_call_written_paths(
                program,
                expression,
                machine,
                machine_symbols,
                symbols,
                &mut active_states,
                &mut expression_writes,
            )?;
            for relative in expression_writes {
                let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
        }
        match statement {
            StatementNode::AssemblyFact(_) | StatementNode::Expression(_) => {}
            StatementNode::Assignment(assignment) => {
                let direct_target = coarse_place_path(program, assignment.target);
                if let Some(relative) = direct_target.as_deref()
                    && rebind_stable_local_mutable_alias_origin(
                        program,
                        machine,
                        state,
                        relative,
                        assignment.value,
                        &mut local_alias_origins,
                        |aliases| {
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                machine_symbols,
                                &mut active_states,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                            )
                        },
                    )?
                {
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    machine_symbols,
                    &mut active_states,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
            StatementNode::Call(call) => {
                let receiver_members = program
                    .statement_table
                    .name_path_members(call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let arguments = program.statement_table.expression_handles(call.arguments);
                let argument_origins = arguments
                    .iter()
                    .map(|argument| {
                        stable_alias_initializer_origin(
                            program,
                            machine,
                            machine_symbols,
                            &mut active_states,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    &receiver_members,
                    arguments,
                    machine,
                    machine_symbols,
                    symbols,
                    &mut active_states,
                    Some(&argument_origins),
                    &mut Vec::new(),
                )
                .or_else(|| {
                    (!arguments
                        .iter()
                        .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
                    .then(|| {
                        known_boundary_call_written_paths_for_parts(
                            program,
                            machine_symbols,
                            symbols,
                            &receiver_members,
                            call.target.as_str(),
                            arguments,
                        )
                    })
                    .flatten()
                })
                .or_else(|| syntactic_call_written_paths(program, &receiver_members, arguments))?;
                for relative in nested_writes {
                    let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                    push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    append_permuted_cycle_frame_edge(program, machine, state, target, &mut edges)?;
                }
            }
            StatementNode::LocalData(local) => {
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                {
                    let origin = declared_local_alias_origin?;
                    local_alias_origins.push((local.name.as_str().to_owned(), origin));
                }
                if type_is_caller_isolated_local(program, local.type_reference) {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    Some(PermutedCycleFrameEquation {
        state,
        locals,
        local_alias_origins,
        direct_writes,
        edges,
    })
}

fn transition_is_exact_write_parameter_permutation(
    program: &TypedTrees,
    source: &State,
    target: &State,
    arguments: &[ExpressionHandle],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> bool {
    let source_write_parameters = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self && parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();
    let target_parameters = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let target_write_positions = target_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();
    if source_write_parameters.len() != target_write_positions.len()
        || target_parameters.len() != arguments.len()
    {
        return false;
    }

    let mut forwarded = Vec::new();
    for (position, _) in target_write_positions {
        let Some(source_parameter) = source_write_parameters.iter().find(|parameter| {
            expression_forwards_exact_write_parameter(
                program,
                arguments[position],
                parameter,
                aliases,
                symbols,
                active_states,
            )
        }) else {
            return false;
        };
        if forwarded.contains(&source_parameter.symbol) {
            return false;
        }
        forwarded.push(source_parameter.symbol);
    }
    forwarded.len() == source_write_parameters.len()
}

fn expression_forwards_exact_write_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> bool {
    if expression_forwards_exact_symbol(program, expression, parameter.symbol) {
        return true;
    }
    if transparent_place_expression_origin(program, expression, symbols, active_states).is_some_and(
        |origin| {
            origin.precision == FramePathPrecision::Exact && origin.path == parameter.name.as_str()
        },
    ) {
        return true;
    }
    let Some(argument) = frame_place_path(program, expression) else {
        return false;
    };
    let (root, suffix) = split_place_root(&argument.path);
    suffix.is_empty()
        && argument.precision == FramePathPrecision::Exact
        && aliases.iter().any(|(alias, origin)| {
            alias == root
                && origin.precision == FramePathPrecision::Exact
                && origin.path == parameter.name.as_str()
        })
}

#[allow(clippy::too_many_arguments)]
/// Summarize one tail transition in the source state's namespace. Named target
/// states compose only when their complete state graph is acyclic; target
/// parameters substitute through authored arguments exactly like call-frame
/// instantiation. Value-position call writes are collected before the jump.
fn summarize_transition_target_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    source_state: &State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    source_locals: &[String],
) -> Option<Vec<String>> {
    if !target.is_valid() {
        return Some(Vec::new());
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Terminal => Some(Vec::new()),
        TransitionTargetNode::Value(_) => Some(Vec::new()),
        // A bare `-> self` re-enters this exact state with the same receiver
        // and parameter namespace. The body's writes have already been
        // collected, so another iteration adds no new caller-visible path.
        // Named cycles remain opaque below unless every edge around the cycle
        // forwards the complete state-parameter namespace positionally without
        // rebinding.
        TransitionTargetNode::SelfTarget => Some(Vec::new()),
        TransitionTargetNode::Named { arguments, .. } => {
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_state =
                named_transition_target_state(program, machine, source_state, target)?;
            if active_states.contains(&target_state.symbol) {
                return named_transition_preserves_state_namespace(
                    program,
                    source_state,
                    target_state,
                    arguments,
                )
                .then(Vec::new);
            }
            active_states.push(target_state.symbol);
            let target_writes = summarize_state_written_paths(
                program,
                machine,
                target_state,
                symbols,
                active_states,
                complete_state_summaries,
            );
            active_states.pop();
            let target_writes = target_writes?;
            let parameters = program.state_parameters(target_state);
            let mut instantiated = Vec::new();
            for relative in target_writes {
                if let Some(path) = instantiate_written_path(
                    program,
                    &relative,
                    Some("self"),
                    parameters,
                    arguments,
                    source_locals,
                    symbols,
                    active_states,
                )? && !instantiated.contains(&path)
                {
                    instantiated.push(path);
                }
            }
            Some(instantiated)
        }
    }
}
