//! Conservative caller-visible write-frame inference.
//!
//! This module owns call-demand collection, internal and boundary call-frame
//! summaries, alias-origin propagation, and transition-cycle frame equations.
//! It produces complete caller-visible paths or fails closed as opaque; call
//! validity and type diagnostics remain owned by the parent module.

use super::receiver_member_chain;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableCall, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod aggregate_results;
mod alias_bindings;
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
mod path_instantiation;
mod place_paths;
mod reference_origins;
mod reference_subjects;
mod state_paths;
mod stored_origins;
mod transition_equations;
mod transition_topology;
mod transparent_effects;
mod type_capabilities;
mod value_expressions;

pub use alias_bindings::state_reference_parameter_binding_is_stable;
use alias_bindings::{
    rebind_stable_local_mutable_alias_origin, stable_local_mutable_alias_rebinding_is_representable,
};
use assignment_targets::{
    assignment_target_type, expression_is_effectful_indexed_place,
    transparent_assignment_target_effect_is_structural,
};
pub(crate) use boundary_calls::boundary_trait_signature;
use boundary_calls::known_boundary_call_written_paths_for_parts;
use call_targets::discarded_primitive_internal_call_is_relationally_neutral;
pub(crate) use call_targets::free_machine_entry_state;
pub(crate) use call_targets::machine_state_by_symbol;
use call_trees::{
    parameter_relative_expression_preserves_transparent_result,
    stable_alias_index_expression_preserves_origin,
};
pub use caller_aliases::{AssignmentWriteTarget, LocalWriteOrigin};
pub(crate) use demand::statement_value_expression_roots;
pub use demand::{CallFrameResolver, frame_paths_overlap};
use demand::{collect_expression_call_written_paths, syntactic_call_written_paths};
use inference::FrameInference;
use isolated_initializers::isolated_local_initializer_preserves_transparent_result;
use isolation::type_is_caller_isolated_local;
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
    FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, append_place_suffix, coarse_place_path,
    frame_place_path, split_place_root,
};
use reference_origins::receiver_frame_origin;
use state_paths::{
    expression_forwards_exact_symbol, normalize_state_relative_path, push_visible_frame_path,
    relative_state_path_is_visible,
};
use stored_origins::{StoredLocalOrigins, expand_write_path};
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
use value_expressions::{ValuePosition, value_expression_preserves_transparent_result};

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
    receiver_origin: Option<&FramePlaceOrigin>,
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let mut complete_state_summaries = Vec::new();
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
        &mut complete_state_summaries,
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
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
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
    let relative_paths = summarize_state_written_paths(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
        complete_state_summaries,
    )
    .or_else(|| {
        summarize_state_written_paths_with_permuted_cycles(
            program,
            callee_machine,
            callee_state,
            symbols,
            inference,
        )
    });
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

fn summarize_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    if let Some((_, paths)) = complete_state_summaries
        .iter()
        .find(|(symbol, _)| *symbol == state.symbol)
    {
        return Some(paths.clone());
    }
    let prefix = walk_state_write_prefix(
        program,
        machine,
        state,
        symbols,
        inference,
        complete_state_summaries,
        None,
    )?;
    complete_state_summaries.push((state.symbol, prefix.written.clone()));
    Some(prefix.written)
}

struct StateWritePrefix {
    written: Vec<String>,
    aliases: Vec<(String, FramePlaceOrigin)>,
    stored: Vec<StoredLocalOrigins>,
    assignment: Option<AssignmentWriteTarget>,
}

enum StateWriteQuery<'statement> {
    Before(&'statement StatementNode),
    ReferenceBefore(&'statement StatementNode),
    Assignment(&'statement StatementNode),
}

/// The same state transfer computes whole-body summaries and the alias context
/// immediately before a demand query. Prefix results never enter the complete
/// state-summary cache.
fn walk_state_write_prefix(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    query: Option<StateWriteQuery<'_>>,
) -> Option<StateWritePrefix> {
    inference.with_local_scope(|inference| {
        walk_state_write_prefix_inner(
            program,
            machine,
            state,
            symbols,
            inference,
            complete_state_summaries,
            query,
        )
    })
}

fn walk_state_write_prefix_inner(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    query: Option<StateWriteQuery<'_>>,
) -> Option<StateWritePrefix> {
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut stored = Vec::new();
    let mut written = Vec::new();
    let include_shared = matches!(query, Some(StateWriteQuery::ReferenceBefore(_)));

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        if matches!(query, Some(StateWriteQuery::Before(before) | StateWriteQuery::ReferenceBefore(before)) if std::ptr::eq(before, statement))
        {
            return Some(StateWritePrefix {
                written,
                aliases: local_alias_origins,
                stored,
                assignment: None,
            });
        }
        let queried_assignment = matches!(query,
            Some(StateWriteQuery::Assignment(candidate)) if std::ptr::eq(candidate, statement));
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference))
                    || (include_shared
                        && type_reference_is_reference(program, local.type_reference)) =>
            {
                stable_local_reference_alias_origin(
                    program,
                    machine,
                    &machine_symbols,
                    inference,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                    &stored,
                    include_shared,
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
                            if include_shared {
                                return reference_subjects::initializer_origin(
                                    program,
                                    machine,
                                    assignment.value,
                                    symbols,
                                    inference,
                                    aliases,
                                    &stored,
                                );
                            }
                            stable_alias_initializer_origin(
                                program,
                                machine,
                                &machine_symbols,
                                inference,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                                &stored,
                            )
                        },
                    )
                }),
            _ => false,
        };
        let declared_stored_origins = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                    && declared_local_alias_origin.is_none() =>
            {
                stored_origins::declaration_origins_for_query(
                    program,
                    machine,
                    local,
                    &local_alias_origins,
                    &stored,
                    symbols,
                    inference,
                    include_shared,
                )
            }
            _ => None,
        };
        if stored_origins::statement_exposes_frozen_binding(
            program, machine, state, statement, &stored,
        ) {
            return None;
        }
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_local_alias_binding(program, expression, &local_alias_origins)
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
                && !alias_bindings::statement_returns_reference_without_effects(
                    program, state, statement,
                )
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
                inference,
                &mut expression_writes,
            )?;
            for relative in expression_writes
                .iter()
                .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
            {
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
                if stored_origins::assignment_replaces_case_binding(program, assignment, &stored)
                    || alias_bindings::assignment_replaces_untracked_reference(
                        program,
                        machine,
                        state,
                        assignment,
                        &local_alias_origins,
                    )
                {
                    return None;
                }
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
                            let origin = if include_shared {
                                reference_subjects::initializer_origin(
                                    program,
                                    machine,
                                    assignment.value,
                                    symbols,
                                    inference,
                                    aliases,
                                    &stored,
                                )
                            } else {
                                stable_alias_initializer_origin(
                                    program,
                                    machine,
                                    &machine_symbols,
                                    inference,
                                    assignment.value,
                                    parameters,
                                    &isolated_local_roots,
                                    aliases,
                                    symbols,
                                    true,
                                    &stored,
                                )
                            };
                            origin.or_else(|| {
                                if !include_shared {
                                    return None;
                                }
                                let reference = crate::places::declared_place_type_raw(
                                    program,
                                    machine,
                                    Some(state),
                                    assignment.target,
                                )?;
                                reference_subjects::unknown_readonly_origin(
                                    program, reference, relative,
                                )
                            })
                        },
                    )?
                {
                    if queried_assignment {
                        return Some(StateWritePrefix {
                            written,
                            aliases: local_alias_origins,
                            stored,
                            assignment: Some(AssignmentWriteTarget::LocalBindingReplacement {
                                path: relative.to_owned(),
                            }),
                        });
                    }
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    &machine_symbols,
                    inference,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                let paths = expand_write_path(&relative, &local_alias_origins, &stored);
                if queried_assignment {
                    return Some(StateWritePrefix {
                        written,
                        aliases: local_alias_origins,
                        stored,
                        assignment: Some(AssignmentWriteTarget::Storage { paths }),
                    });
                }
                for path in paths {
                    push_visible_frame_path(&mut written, path, parameters, &locals)?;
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
                            inference,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                            &stored,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins(
                    program,
                    nested_call.target_symbol,
                    nested_call.target.as_str(),
                    &nested_receiver_members,
                    None,
                    arguments,
                    machine,
                    &machine_symbols,
                    symbols,
                    inference,
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
                            machine,
                            &machine_symbols,
                            symbols,
                            &nested_receiver_members,
                            nested_call.target.as_str(),
                            caller_aliases::CallerWriteSite::Call(nested_call),
                            arguments,
                            inference,
                        )
                    })
                    .flatten()
                })
                .or_else(|| {
                    syntactic_call_written_paths(
                        program,
                        &nested_receiver_members,
                        arguments,
                        &machine_symbols,
                        symbols,
                    )
                })?;
                for relative in nested_writes
                    .iter()
                    .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
                {
                    if relative_state_path_is_visible(&relative, parameters, &locals)?
                        && !written.contains(&relative)
                    {
                        written.push(relative);
                    }
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    if (!local_alias_origins.is_empty() || !stored.is_empty())
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
                        inference,
                        complete_state_summaries,
                        &locals,
                    )?
                    .iter()
                    .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
                    {
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
                if (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference))
                    || (include_shared
                        && type_reference_is_reference(program, local.type_reference))
                {
                    // An unknown read-only origin is local to this query. Add
                    // it only after the exposure/effect checks: retaining a
                    // binding is not admission to expose another alias slot.
                    let origin = declared_local_alias_origin.or_else(|| {
                        if !include_shared {
                            return None;
                        }
                        reference_subjects::unknown_readonly_origin(
                            program,
                            local.type_reference,
                            local.name.as_str(),
                        )
                    });
                    if let Some(origin) = origin {
                        local_alias_origins.push((local.name.as_str().to_owned(), origin));
                    } else {
                        let origins = declared_stored_origins?;
                        inference.record_local(&origins);
                        stored.push(origins);
                    }
                } else if stored_origins::has_aggregate_case_shape(program, local.type_reference)
                    && let Some(origins) = stored_origins::declaration_origins(
                        program,
                        machine,
                        local,
                        &local_alias_origins,
                        &stored,
                        symbols,
                        inference,
                    )
                {
                    // Failure to recover a no-write value's cases does not
                    // invalidate its writes. A later payload projection still
                    // requires positive case evidence from this same transfer.
                    inference.record_local(&origins);
                    stored.push(origins);
                }
                if type_is_caller_isolated_local(program, local.type_reference)
                    && !local_alias_origins
                        .iter()
                        .any(|(name, _)| name == local.name.as_str())
                {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    query.is_none().then_some(StateWritePrefix {
        written,
        aliases: local_alias_origins,
        stored,
        assignment: None,
    })
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
fn stable_local_reference_alias_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    local: &typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    stored: &[StoredLocalOrigins],
    include_shared: bool,
) -> Option<FramePlaceOrigin> {
    let mut reference = local.type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    let TypeReferenceNode::Reference { access, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !access.is_exclusive() && !include_shared {
        return None;
    }
    if include_shared {
        return reference_subjects::initializer_origin(
            program,
            current_machine,
            local.initial_value,
            symbols,
            inference,
            aliases,
            stored,
        );
    }
    stable_alias_initializer_origin(
        program,
        current_machine,
        machine_symbols,
        inference,
        local.initial_value,
        parameters,
        isolated_local_roots,
        aliases,
        symbols,
        true,
        stored,
    )
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_initializer_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    inference: &mut FrameInference,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            inference,
            inner.target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
            stored,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_initializer_origin(
                    program,
                    current_machine,
                    machine_symbols,
                    inference,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                    stored,
                );
            }
            transparent_call_result_origin(
                program,
                call,
                symbols,
                inference,
                |_, _, _, actual, inference| {
                    stable_alias_initializer_origin(
                        program,
                        current_machine,
                        machine_symbols,
                        inference,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        symbols,
                        allow_isolated_local,
                        stored,
                    )
                },
            )
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index)
                && !stable_alias_index_expression_preserves_origin(
                    program,
                    current_machine,
                    indexed.index,
                    machine_symbols,
                    symbols,
                    inference,
                    parameters,
                    aliases,
                )
            {
                return None;
            }
            let mut collection = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            collection.source =
                collection
                    .source
                    .projected(program, expression, indexed.collection);
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )?;
            let source = receiver
                .source
                .projected(program, expression, member.receiver);
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..receiver },
            })
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                inference,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
                stored,
            )
        }
        _ => stable_alias_expression_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        )
        .or_else(|| {
            let origin = frame_place_path(program, expression)?;
            // Stored carrier slots cannot be replaced while this transfer is
            // complete. Keep their symbolic source: a dynamic projection may
            // reach several leaves, so freezing must not select just one.
            stored
                .iter()
                .any(|local| local.local_symbol == origin.source.root)
                .then_some(origin)
        }),
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
            transparent_call_result_origin(
                program,
                call,
                symbols,
                &mut FrameInference::default(),
                |_, _, _, actual, _| {
                    stable_alias_expression_origin(
                        program,
                        actual,
                        parameters,
                        isolated_local_roots,
                        aliases,
                        symbols,
                        allow_isolated_local,
                    )
                },
            )
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
            collection.source =
                collection
                    .source
                    .projected(program, expression, indexed.collection);
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
            let source = receiver
                .source
                .projected(program, expression, member.receiver);
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..receiver },
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
    inference: &mut FrameInference,
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
            inference,
            target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            true,
            &[],
        )?
        .path,
    )
}

/// Recover one deliberately structural value-call relation. The helper may be
/// free or attached, but must be acyclic at the result surface, return a reference,
/// and have one terminal result expression rooted in one reference
/// parameter. A prefix may contain caller-isolated scratch locals and local
/// reference bindings that forward direct places from that parameter, an
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
/// unsupported discarded/statement call, recursive helper relation, named-state route, or
/// alternate result fails closed.
fn transparent_call_result_origin(
    program: &TypedTrees,
    call: &TableCallExpression,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    resolve_actual_origin: impl FnOnce(
        &Machine,
        &StateParameter,
        &FramePlaceOrigin,
        ExpressionHandle,
        &mut FrameInference,
    ) -> Option<FramePlaceOrigin>,
) -> Option<FramePlaceOrigin> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }

    let result_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
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
    let argument_origin = resolve_actual_origin(
        callee_machine,
        result_parameter,
        &result_origin.place,
        actual,
        inference,
    )?;
    let source = argument_origin
        .source
        .append_relative(&result_origin.place.source);
    Some(match argument_origin.precision {
        FramePathPrecision::Exact => FramePlaceOrigin {
            path: append_place_suffix(&argument_origin.path, result_suffix),
            precision: result_origin.place.precision,
            source,
        },
        FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
            source,
            ..argument_origin
        },
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
    inference: &mut FrameInference,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            transparent_place_expression_origin(program, inner.target, symbols, inference)
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut origin = transparent_place_expression_origin(
                program,
                indexed.collection,
                symbols,
                inference,
            )?;
            origin.source = origin
                .source
                .projected(program, expression, indexed.collection);
            origin.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let origin =
                transparent_place_expression_origin(program, member.receiver, symbols, inference)?;
            let source = origin
                .source
                .projected(program, expression, member.receiver);
            Some(match origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", origin.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin { source, ..origin },
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return transparent_place_expression_origin(
                    program,
                    call.receiver,
                    symbols,
                    inference,
                );
            }
            transparent_call_result_origin(
                program,
                call,
                symbols,
                inference,
                |_, _, _, actual, inference| {
                    transparent_place_expression_origin(program, actual, symbols, inference)
                },
            )
        }
        _ => frame_place_path(program, expression),
    }
}

fn transparent_callee_result_origin(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    if inference.active_states.contains(&callee_state.symbol)
        || !type_reference_is_reference(program, callee_state.return_type)
    {
        return None;
    }
    inference.active_states.push(callee_state.symbol);
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
                    let stable_aliases = local_aliases
                        .iter()
                        .map(
                            |(name, _, origin): &(
                                String,
                                SymbolHandle,
                                ParameterRelativeFrameOrigin,
                            )| { (name.clone(), origin.place.clone()) },
                        )
                        .collect::<Vec<_>>();
                    if type_is_caller_isolated_local(program, local.type_reference)
                        && !type_reference_is_reference(program, local.type_reference)
                    {
                        if expression_is_effectful_for_transparent_result(
                            program,
                            local.initial_value,
                        ) && !value_expression_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            Some(local.type_reference),
                            symbols,
                            inference,
                            parameters,
                            &local_aliases,
                        ) {
                            return None;
                        }
                        if !isolated_local_initializer_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            &isolated_local_roots,
                            &stable_aliases,
                            |machine_symbols, written| {
                                collect_expression_call_written_paths(
                                    program,
                                    local.initial_value,
                                    callee_machine,
                                    machine_symbols,
                                    symbols,
                                    inference,
                                    written,
                                )
                            },
                        ) {
                            return None;
                        }
                        isolated_local_roots.push(local.name.as_str().to_owned());
                        continue;
                    }
                    if !type_reference_is_reference(program, local.type_reference) {
                        return None;
                    }
                    let origin = parameter_relative_place_origin(
                        program,
                        callee_machine,
                        local.initial_value,
                        parameters,
                        &local_aliases,
                        symbols,
                        inference,
                    )
                    .or_else(|| {
                        let mut diagnostics = Vec::new();
                        let machine_symbols =
                            MachineSymbols::build(program, callee_machine, &mut diagnostics);
                        if !diagnostics.is_empty() {
                            return None;
                        }
                        let place = stable_alias_initializer_origin(
                            program,
                            callee_machine,
                            &machine_symbols,
                            inference,
                            local.initial_value,
                            parameters,
                            &isolated_local_roots,
                            &stable_aliases,
                            symbols,
                            true,
                            &[],
                        )?;
                        let (root, _) = split_place_root(&place.path);
                        isolated_local_roots
                            .iter()
                            .any(|local| local == root)
                            .then_some(ParameterRelativeFrameOrigin {
                                place,
                                parameter_symbol: SymbolHandle::invalid(),
                            })
                    })?;
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
                            inference,
                        )
                        .is_none())
                    {
                        return None;
                    }
                    if value_expression_preserves_transparent_result(
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
                        inference,
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
                            inference,
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
                        inference,
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
            inference,
        )
        .filter(|origin| {
            origin.parameter_symbol.is_valid()
                && parameters.iter().any(|parameter| {
                    parameter.symbol == origin.parameter_symbol
                        && (origin.place.source.root == parameter.symbol
                            || (parameter.is_self
                                && origin.place.source.root == callee_machine.symbol))
                })
        })
    })();
    inference.active_states.pop();
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
    inference: &mut FrameInference,
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
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    let argument_types = call_targets::call_argument_types(
        program,
        current_machine,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        caller_aliases::CallerWriteSite::Call(call),
        &machine_symbols,
        symbols,
    );
    // Every sibling must independently preserve the returned-place origin.
    if arguments.iter().enumerate().any(|(index, argument)| {
        !parameter_relative_expression_preserves_transparent_result(
            program,
            current_machine,
            *argument,
            ValuePosition::CallArgument(argument_types.get(index).copied().unwrap_or_default()),
            &machine_symbols,
            symbols,
            inference,
            parameters,
            aliases,
        )
    }) {
        return false;
    }

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
                inference,
            )
            .map(|origin| origin.place)
        })
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts_with_origins(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        None,
        arguments,
        current_machine,
        &machine_symbols,
        symbols,
        inference,
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
                current_machine,
                &machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                caller_aliases::CallerWriteSite::Call(call),
                arguments,
                inference,
            )
        })
        .flatten()
    })
    .is_some()
}

fn parameter_relative_place_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => parameter_relative_place_origin(
            program,
            current_machine,
            inner.target,
            parameters,
            aliases,
            symbols,
            inference,
        ),
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                let mut diagnostics = Vec::new();
                let machine_symbols =
                    MachineSymbols::build(program, current_machine, &mut diagnostics);
                if !diagnostics.is_empty()
                    || !parameter_relative_expression_preserves_transparent_result(
                        program,
                        current_machine,
                        indexed.index,
                        ValuePosition::IndexOperand,
                        &machine_symbols,
                        symbols,
                        inference,
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
                inference,
            )?;
            origin.place.source =
                origin
                    .place
                    .source
                    .projected(program, expression, indexed.collection);
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
                inference,
            )?;
            if origin.place.precision == FramePathPrecision::Exact {
                origin.place.path = format!("{}.{}", origin.place.path, member.member.as_str());
            }
            origin.place.source =
                origin
                    .place
                    .source
                    .projected(program, expression, member.receiver);
            Some(origin)
        }
        ExpressionNode::Name(_) => {
            reference_origins::declared_origin_root(program, current_machine, expression)?;
            let place = frame_place_path(program, expression)?;
            let root_symbol = frame_place_root_symbol(program, expression);
            let (root, suffix) = split_place_root(&place.path);
            if let Some(parameter) = parameters.iter().find(|parameter| {
                (root_symbol == Some(parameter.symbol) || (parameter.is_self && root == "self"))
                    && type_reference_is_reference(program, parameter.type_reference)
            }) {
                return Some(ParameterRelativeFrameOrigin {
                    place,
                    parameter_symbol: parameter.symbol,
                });
            }
            let parent = aliases.iter().find_map(|(_, symbol, origin)| {
                let exact_symbol = root_symbol
                    .is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
                exact_symbol.then_some(origin)
            })?;
            let source = parent.place.source.append_segments(&place.source.segments);
            Some(ParameterRelativeFrameOrigin {
                place: match parent.place.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&parent.place.path, suffix),
                        precision: place.precision,
                        source,
                    },
                    FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                        source,
                        ..parent.place.clone()
                    },
                },
                parameter_symbol: parent.parameter_symbol,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                let mut origin = parameter_relative_place_origin(
                    program,
                    current_machine,
                    call.receiver,
                    parameters,
                    aliases,
                    symbols,
                    inference,
                )?;
                // An unresolved view builtin supplies a conservative collection
                // footprint, not a nominal helper-result identity. A same-name
                // unresolved user method cannot establish an exact subject.
                origin.place.precision = FramePathPrecision::CollectionCoarse;
                return Some(origin);
            }
            parameter_relative_call_result_origin(
                program,
                current_machine,
                call,
                parameters,
                aliases,
                symbols,
                inference,
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
                inference,
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
    inference: &mut FrameInference,
) -> Option<ParameterRelativeFrameOrigin> {
    if std::iter::once(call.receiver)
        .chain(
            program
                .expression_table
                .expression_handles(call.arguments)
                .iter()
                .copied(),
        )
        .any(|expression| {
            expression_reborrows_transparent_alias_binding(
                program,
                expression,
                caller_parameters,
                caller_aliases,
            )
        })
    {
        return None;
    }
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }
    let callee_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        inference,
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
        inference,
    )?;
    let (_, suffix) = split_place_root(&callee_origin.place.path);
    let source = actual_origin
        .place
        .source
        .append_relative(&callee_origin.place.source);
    Some(match actual_origin.place.precision {
        FramePathPrecision::Exact => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                path: append_place_suffix(&actual_origin.place.path, suffix),
                precision: callee_origin.place.precision,
                source,
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
        FramePathPrecision::CollectionCoarse => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                source,
                ..actual_origin.place
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
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
    outer_inference: &FrameInference,
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
            outer_inference,
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
                let mut inference = outer_inference.clone();
                if !transition_is_exact_write_parameter_permutation(
                    program,
                    equation.state,
                    target,
                    &edge.arguments,
                    &equation.local_alias_origins,
                    symbols,
                    &mut inference,
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
            let mut inference = outer_inference.clone();
            for local in &equation.stored {
                inference.record_local(local);
            }
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
                    for instantiated in instantiate_written_path(
                        program,
                        machine,
                        &relative,
                        Some("self"),
                        program.state_parameters(target.state),
                        &edge.arguments,
                        &equation.locals,
                        symbols,
                        &mut inference,
                    )?
                    .iter()
                    .flat_map(|path| {
                        expand_write_path(path, &equation.local_alias_origins, &equation.stored)
                    }) {
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
    outer_inference: &FrameInference,
) -> Option<PermutedCycleFrameEquation<'program>> {
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut stored = Vec::new();
    let mut direct_writes = Vec::new();
    let mut edges = Vec::new();
    let mut inference = outer_inference.clone();
    if !inference.active_states.contains(&state.symbol) {
        inference.active_states.push(state.symbol);
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference) =>
            {
                stable_local_reference_alias_origin(
                    program,
                    machine,
                    machine_symbols,
                    &mut inference,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                    &stored,
                    false,
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
                                &mut inference,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                                &stored,
                            )
                        },
                    )
                }),
            _ => false,
        };
        if stored_origins::statement_exposes_frozen_binding(
            program, machine, state, statement, &stored,
        ) {
            return None;
        }
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
                &mut inference,
                &mut expression_writes,
            )?;
            for relative in expression_writes
                .iter()
                .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
            {
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
        }
        match statement {
            StatementNode::AssemblyFact(_) | StatementNode::Expression(_) => {}
            StatementNode::Assignment(assignment) => {
                if stored_origins::assignment_replaces_case_binding(program, assignment, &stored)
                    || alias_bindings::assignment_replaces_untracked_reference(
                        program,
                        machine,
                        state,
                        assignment,
                        &local_alias_origins,
                    )
                {
                    return None;
                }
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
                                &mut inference,
                                assignment.value,
                                parameters,
                                &isolated_local_roots,
                                aliases,
                                symbols,
                                true,
                                &stored,
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
                    &mut inference,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                for path in expand_write_path(&relative, &local_alias_origins, &stored) {
                    push_visible_frame_path(&mut direct_writes, path, parameters, &locals)?;
                }
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
                            &mut inference,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                            &stored,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    &receiver_members,
                    None,
                    arguments,
                    machine,
                    machine_symbols,
                    symbols,
                    &mut inference,
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
                            machine,
                            machine_symbols,
                            symbols,
                            &receiver_members,
                            call.target.as_str(),
                            caller_aliases::CallerWriteSite::Call(call),
                            arguments,
                            &mut inference,
                        )
                    })
                    .flatten()
                })
                .or_else(|| {
                    syntactic_call_written_paths(
                        program,
                        &receiver_members,
                        arguments,
                        machine_symbols,
                        symbols,
                    )
                })?;
                for relative in nested_writes
                    .iter()
                    .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
                {
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
                    if let Some(origin) = declared_local_alias_origin {
                        local_alias_origins.push((local.name.as_str().to_owned(), origin));
                    } else {
                        let origins = stored_origins::declaration_origins(
                            program,
                            machine,
                            local,
                            &local_alias_origins,
                            &stored,
                            symbols,
                            &mut inference,
                        )?;
                        inference.record_local(&origins);
                        stored.push(origins);
                    }
                } else if stored_origins::has_aggregate_case_shape(program, local.type_reference)
                    && let Some(origins) = stored_origins::declaration_origins(
                        program,
                        machine,
                        local,
                        &local_alias_origins,
                        &stored,
                        symbols,
                        &mut inference,
                    )
                {
                    inference.record_local(&origins);
                    stored.push(origins);
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
        stored,
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
    inference: &mut FrameInference,
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
                inference,
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
    inference: &mut FrameInference,
) -> bool {
    if expression_forwards_exact_symbol(program, expression, parameter.symbol) {
        return true;
    }
    if transparent_place_expression_origin(program, expression, symbols, inference).is_some_and(
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
    target: typed_trees::statement::TransitionTargetHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
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
            if inference.active_states.contains(&target_state.symbol) {
                return named_transition_preserves_state_namespace(
                    program,
                    source_state,
                    target_state,
                    arguments,
                )
                .then(Vec::new);
            }
            inference.active_states.push(target_state.symbol);
            let target_writes = summarize_state_written_paths(
                program,
                machine,
                target_state,
                symbols,
                inference,
                complete_state_summaries,
            );
            inference.active_states.pop();
            let target_writes = target_writes?;
            let parameters = program.state_parameters(target_state);
            let mut instantiated = Vec::new();
            for relative in target_writes {
                for path in instantiate_written_path(
                    program,
                    machine,
                    &relative,
                    Some("self"),
                    parameters,
                    arguments,
                    source_locals,
                    symbols,
                    inference,
                )? {
                    if !instantiated.contains(&path) {
                        instantiated.push(path);
                    }
                }
            }
            Some(instantiated)
        }
    }
}
