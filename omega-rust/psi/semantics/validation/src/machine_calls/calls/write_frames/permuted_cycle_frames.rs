//! Transition-cycle frame equations over exact write-parameter
//! permutations and transition-target write summaries.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::alias_bindings::{
    rebind_stable_local_mutable_alias_origin, stable_local_mutable_alias_rebinding_is_representable,
};
use crate::machine_calls::calls::write_frames::alias_origins::{
    stable_alias_initializer_origin, stable_alias_initializer_origins,
    stable_assignment_target_path, stable_local_reference_alias_origin,
};
use crate::machine_calls::calls::write_frames::assignment_targets::expression_is_effectful_indexed_place;
use crate::machine_calls::calls::write_frames::boundary_calls::known_boundary_call_written_paths_for_parts;
use crate::machine_calls::calls::write_frames::caller_aliases::CallerWriteSite;
use crate::machine_calls::calls::write_frames::demand::{
    collect_expression_call_written_paths, statement_value_expression_roots,
    syntactic_call_written_paths,
};
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::isolation::type_is_caller_isolated_local;
use crate::machine_calls::calls::write_frames::known_call_written_paths_for_parts_with_origins;
use crate::machine_calls::calls::write_frames::local_aliases::expression_reborrows_unresolved_reference_binding;
use crate::machine_calls::calls::write_frames::path_instantiation::instantiate_written_path;
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, coarse_place_path, frame_place_path, split_place_root,
};
use crate::machine_calls::calls::write_frames::state_paths::{
    expression_forwards_exact_symbol, push_visible_frame_path, relative_state_path_is_visible,
};
use crate::machine_calls::calls::write_frames::state_write_walk::summarize_state_written_paths;
use crate::machine_calls::calls::write_frames::stored_origins::expand_write_path;
use crate::machine_calls::calls::write_frames::transition_equations::{
    PermutedCycleFrameEquation, append_permuted_cycle_frame_edge, transition_state_reaches,
};
use crate::machine_calls::calls::write_frames::transition_topology::{
    named_transition_preserves_state_namespace, named_transition_target_state,
    reachable_cycle_edges_can_permute_write_parameters, write_parameter_counts_can_permute,
};
use crate::machine_calls::calls::write_frames::transparent_results::transparent_place_expression_origin;
use crate::machine_calls::calls::write_frames::type_capabilities::{
    parameter_may_carry_write, type_may_carry_write,
};
use crate::machine_calls::calls::write_frames::{alias_bindings, stored_origins, wire_codecs};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

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
pub(crate) fn summarize_state_written_paths_with_permuted_cycles<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'program>,
    outer_inference: &FrameInference,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    // The prefix walk below re-enters this solver at every nested visit of a
    // state that reaches a named cycle. When a cyclic edge cannot pair the
    // write-capable parameters by count, the solve is doomed before any
    // equation exists; deciding that from the topology keeps a declined
    // system from rebuilding every reachable equation, and every nested call
    // inside them, once per visit. The answer is the same `None` the
    // permutation check would have produced.
    if !reachable_cycle_edges_can_permute_write_parameters(program, machine, entry) {
        return None;
    }
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
            complete_state_summaries,
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
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<PermutedCycleFrameEquation<'program>> {
    #[cfg(test)]
    super::CYCLE_EQUATIONS.with(|equations| equations.set(equations.get() + 1));
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
                    &[],
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
                                &[],
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
            program,
            machine,
            state,
            statement,
            &stored,
            &local_alias_origins,
        ) {
            return None;
        }
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_unresolved_reference_binding(
                program,
                machine,
                expression,
                parameters,
                &isolated_local_roots,
                &local_alias_origins,
                &[],
            ) && declared_local_alias_origin.is_none()
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
                complete_state_summaries,
                &super::caller_aliases::CallOriginContext {
                    parameters,
                    isolated_locals: &isolated_local_roots,
                    aliases: &local_alias_origins,
                    divergent: &[],
                    stored: &stored,
                },
            )?;
            for relative in expression_writes
                .iter()
                .flat_map(|path| expand_write_path(path, &local_alias_origins, &stored))
            {
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
        }
        match statement {
            StatementNode::RootBinding(_)
            | StatementNode::AssemblyFact(_)
            | StatementNode::Expression(_) => {}
            StatementNode::Assignment(assignment) => {
                if (!representable_alias_rebinding
                    && stored_origins::assignment_replaces_case_binding(
                        program,
                        assignment,
                        &stored,
                        &local_alias_origins,
                    ))
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
                                &[],
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
                // Synthesized wire codecs frame from their borrowed arguments;
                // the type-name receiver never reaches the ownership floor.
                let nested_writes = if wire_codecs::is_wire_codec_call(program, call) {
                    wire_codecs::known_wire_codec_call_written_paths(program, call)
                } else {
                    let argument_origins = arguments
                        .iter()
                        .map(|argument| {
                            stable_alias_initializer_origins(
                                program,
                                machine,
                                machine_symbols,
                                &mut inference,
                                *argument,
                                parameters,
                                &isolated_local_roots,
                                &local_alias_origins,
                                &[],
                                symbols,
                                true,
                                &stored,
                            )
                        })
                        .collect::<Vec<_>>();
                    known_call_written_paths_for_parts_with_origins(
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
                        (!arguments.iter().any(|argument| {
                            expression_is_effectful_indexed_place(program, *argument)
                        }))
                        .then(|| {
                            known_boundary_call_written_paths_for_parts(
                                program,
                                machine,
                                machine_symbols,
                                symbols,
                                &receiver_members,
                                call.target.as_str(),
                                CallerWriteSite::Call(call),
                                arguments,
                                &mut inference,
                            )
                        })
                        .flatten()
                    })
                    .or_else(|| {
                        syntactic_call_written_paths(
                            program,
                            machine,
                            &receiver_members,
                            arguments,
                            machine_symbols,
                            symbols,
                        )
                    })
                }?;
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
    if !write_parameter_counts_can_permute(program, source, target, arguments) {
        return false;
    }
    let source_write_parameters = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self && parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();
    let target_write_positions = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .filter(|(_, parameter)| parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();

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
pub(crate) fn summarize_transition_target_written_paths(
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

#[cfg(test)]
mod tests;
