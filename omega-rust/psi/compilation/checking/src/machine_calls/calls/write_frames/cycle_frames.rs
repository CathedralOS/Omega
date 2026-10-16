//! Finite transition-cycle write-frame equations over storage-root substitutions.

use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::alias_bindings::{
    rebind_direct_assignment_target, representable_assignment_alias_rebinding,
};
use crate::machine_calls::calls::write_frames::alias_origins::{
    stable_alias_initializer_origin, stable_alias_initializer_origins,
    stable_assignment_target_path, stable_local_reference_alias_origin,
};
use crate::machine_calls::calls::write_frames::assignment_targets::expression_is_effectful_indexed_place;
use crate::machine_calls::calls::write_frames::boundary_calls::known_boundary_call_written_paths_for_parts;
use crate::machine_calls::calls::write_frames::caller_aliases::{
    CallOriginContext, CallerWriteSite,
};
use crate::machine_calls::calls::write_frames::demand::{
    collect_expression_call_written_paths, statement_value_expression_roots,
    syntactic_call_written_paths,
};
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::isolation::type_is_caller_isolated_local;
use crate::machine_calls::calls::write_frames::known_call_written_paths_for_parts;
use crate::machine_calls::calls::write_frames::local_aliases::expression_reborrows_unresolved_reference_binding;
use crate::machine_calls::calls::write_frames::path_instantiation::{
    instantiate_written_path, instantiate_written_path_for_origins,
};
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin,
};
use crate::machine_calls::calls::write_frames::state_paths::{
    push_visible_frame_path, relative_state_path_is_visible,
};
use crate::machine_calls::calls::write_frames::state_write_walk::{
    summarize_complete_state_written_paths, summarize_state_written_paths,
};
use crate::machine_calls::calls::write_frames::stored_origins::{
    expand_write_path, expand_write_paths,
};
use crate::machine_calls::calls::write_frames::transition_equations::{
    CycleFrameEquation, append_cycle_frame_edge, transition_state_reaches,
};
use crate::machine_calls::calls::write_frames::transition_topology::{
    named_state_transition_subgraph_is_acyclic, named_transition_preserves_state_namespace,
    named_transition_target_state,
};
use crate::machine_calls::calls::write_frames::type_capabilities::type_may_carry_write;
use crate::machine_calls::calls::write_frames::{stored_origins, wire_codecs};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

/// Solve complete may-write equations for a named transition graph.
///
/// Edges substitute each target storage root through the actual argument's
/// proven origins. Roots may be introduced, dropped, or shared; a may-write
/// union does not require a parameter bijection. The storage-root dependency
/// graph must prove that repeated substitution cannot grow paths without
/// bound. Unknown effects or origins remain opaque. Borrow admission is a
/// separate judgment and still rejects conflicting uses of aliased storage.
pub(crate) fn summarize_state_written_paths_for_cycles<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'program>,
    outer_inference: &FrameInference,
) -> Option<Vec<String>> {
    // The equations below take their edges from the same top-level named
    // transitions the topology walk follows, so a state whose reachable named
    // subgraph is acyclic can never set `has_transition_cycle`. Answer its
    // `None` before building an equation, and every nested call summary
    // inside one, for each reachable state.
    if named_state_transition_subgraph_is_acyclic(program, machine, entry) {
        return None;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return None;
    }

    let mut equations = Vec::<CycleFrameEquation<'program>>::new();
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
        let equation = build_cycle_frame_equation(
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

    let has_transition_cycle = equations.iter().any(|equation| {
        equation
            .edges
            .iter()
            .any(|edge| transition_state_reaches(&equations, edge.target, equation.state.symbol))
    });
    if !has_transition_cycle
        || !super::cycle_substitutions::substitutions_are_finite(
            program,
            &equations,
            symbols,
            outer_inference,
        )
    {
        return None;
    }

    let mut summaries = equations
        .iter()
        .map(|equation| (equation.state.symbol, equation.direct_writes.clone()))
        .collect::<Vec<_>>();
    let equation_index = equations
        .iter()
        .enumerate()
        .map(|(index, equation)| (equation.state.symbol, index))
        .collect::<std::collections::HashMap<_, _>>();
    // Only a write a state gained since the previous pass can instantiate
    // anything new through an incoming edge, so each pass propagates just the
    // just-arrived writes instead of re-instantiating every held path.
    let mut delta = summaries
        .iter()
        .map(|(_, writes)| writes.clone())
        .collect::<Vec<_>>();
    loop {
        let mut produced = false;
        let mut next_delta = vec![Vec::new(); summaries.len()];
        for (source_index, equation) in equations.iter().enumerate() {
            let mut inference = outer_inference.clone();
            for local in &equation.stored {
                inference.record_local(local);
            }
            for edge in &equation.edges {
                let target_index = *equation_index.get(&edge.target)?;
                let target = &equations[target_index];
                for relative in &delta[target_index] {
                    let instantiated_paths = instantiate_written_path(
                        program,
                        machine,
                        relative,
                        Some("self"),
                        program.state_parameters(target.state),
                        &edge.arguments,
                        &equation.locals,
                        symbols,
                        &mut inference,
                    )?;
                    for instantiated in expand_write_paths(
                        program,
                        &instantiated_paths,
                        &equation.local_alias_origins,
                        &equation.stored,
                    )? {
                        if !relative_state_path_is_visible(
                            &instantiated,
                            program.state_parameters(equation.state),
                            &equation.locals,
                        )? {
                            continue;
                        }
                        let source_writes = &mut summaries[source_index].1;
                        if !source_writes.contains(&instantiated) {
                            source_writes.push(instantiated.clone());
                            next_delta[source_index].push(instantiated);
                            produced = true;
                        }
                    }
                }
            }
        }
        if !produced {
            break;
        }
        delta = next_delta;
    }

    // The fixed point solves every reachable cyclic state at once, and every
    // solved state's reachable region is already inside this equation set —
    // record all of them in the shared memo so the next state's complete-frame
    // query reads this solve instead of re-running the same equation system.
    for (symbol, writes) in &summaries {
        symbols
            .caller_sites
            .record_completed_state_writes(*symbol, writes);
    }
    summaries
        .into_iter()
        .find_map(|(symbol, writes)| (symbol == entry.symbol).then_some(writes))
}

fn build_cycle_frame_equation<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    symbols: &TopLevelSymbols<'program>,
    machine_symbols: &MachineSymbols<'program>,
    outer_inference: &FrameInference,
) -> Option<CycleFrameEquation<'program>> {
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
        let representable_alias_rebinding = representable_assignment_alias_rebinding(
            program,
            machine,
            state,
            statement,
            &local_alias_origins,
            |aliases, value| {
                stable_alias_initializer_origin(
                    program,
                    machine,
                    machine_symbols,
                    &mut inference,
                    value,
                    parameters,
                    &isolated_local_roots,
                    aliases,
                    &[],
                    symbols,
                    true,
                    &stored,
                )
            },
        );
        if stored_origins::statement_exposes_frozen_binding(
            program,
            machine,
            state,
            statement,
            &stored,
            &local_alias_origins,
            symbols,
            &mut inference,
            false,
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
                &super::caller_aliases::CallOriginContext {
                    parameters,
                    isolated_locals: &isolated_local_roots,
                    aliases: &local_alias_origins,
                    divergent: &[],
                    stored: &stored,
                },
            )?;
            for relative in
                expand_write_paths(program, &expression_writes, &local_alias_origins, &stored)?
            {
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
        }
        match statement {
            StatementNode::RootBinding(_)
            | StatementNode::Assembly(_)
            | StatementNode::AssemblyFact(_)
            | StatementNode::Expression(_) => {}
            StatementNode::Assignment(assignment) => {
                let replacement = stored_origins::assignment_origins(
                    program,
                    machine,
                    state,
                    statement,
                    assignment,
                    &local_alias_origins,
                    &stored,
                    symbols,
                    &mut inference,
                    false,
                    representable_alias_rebinding,
                )?;
                if rebind_direct_assignment_target(
                    program,
                    machine,
                    state,
                    assignment,
                    &mut local_alias_origins,
                    |aliases, _| {
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
                .is_some()
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
                for path in expand_write_path(program, &relative, &local_alias_origins, &stored)? {
                    push_visible_frame_path(&mut direct_writes, path, parameters, &locals)?;
                }
                if let Some(replacement) = replacement {
                    replacement.install(&mut inference, &mut stored);
                }
            }
            StatementNode::Call(call) => {
                let receiver_members =
                    crate::machine_calls::calls::signature_queries::call_receiver_member_names(
                        program, call,
                    );
                let arguments = program.statement_table.expression_handles(call.arguments);
                // Synthesized wire codecs frame from their borrowed arguments;
                // the type-name receiver never reaches the ownership floor.
                let nested_writes = if wire_codecs::is_wire_codec_call(program, call) {
                    wire_codecs::known_wire_codec_call_written_paths(
                        program,
                        machine,
                        call,
                        parameters,
                        &isolated_local_roots,
                        &local_alias_origins,
                        &[],
                        symbols,
                        &mut inference,
                    )
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
                    known_call_written_paths_for_parts(
                        program,
                        call.target_symbol,
                        call.target.as_str(),
                        &receiver_members,
                        &[],
                        None,
                        arguments,
                        machine,
                        machine_symbols,
                        symbols,
                        &mut inference,
                        Some(&argument_origins),
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
                for relative in
                    expand_write_paths(program, &nested_writes, &local_alias_origins, &stored)?
                {
                    push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
                }
            }
            StatementNode::Transition(transition) => {
                for target in transition.targets() {
                    // Calls leave this state graph. Their complete writes
                    // contribute to this equation; they are not SCC edges.
                    if super::terminal_calls::target(program, machine, target).is_valid() {
                        let origins = CallOriginContext {
                            parameters,
                            isolated_locals: &isolated_local_roots,
                            aliases: &local_alias_origins,
                            divergent: &[],
                            stored: &stored,
                        };
                        let writes = super::terminal_calls::written_paths(
                            program,
                            machine,
                            target,
                            machine_symbols,
                            symbols,
                            &mut inference,
                            &origins,
                        )?;
                        for relative in
                            expand_write_paths(program, &writes, &local_alias_origins, &stored)?
                        {
                            push_visible_frame_path(
                                &mut direct_writes,
                                relative,
                                parameters,
                                &locals,
                            )?;
                        }
                        continue;
                    }
                    append_cycle_frame_edge(program, machine, state, target, &mut edges)?;
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
                        )
                        .or_else(|| {
                            stored_origins::unresolved_origins(program, machine, local, false)
                        })?;
                        inference.record_local(&origins);
                        stored.push(origins);
                    }
                } else {
                    stored_origins::record_declaration_origins(
                        program,
                        machine,
                        local,
                        &local_alias_origins,
                        &mut stored,
                        symbols,
                        &mut inference,
                    );
                }
                if type_is_caller_isolated_local(program, local.type_reference) {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    Some(CycleFrameEquation {
        state,
        locals,
        local_alias_origins,
        stored,
        direct_writes,
        edges,
    })
}

#[allow(clippy::too_many_arguments)]
/// Summarize one tail transition in the source state's namespace. Complete
/// queries recursively require complete targets; contextual prefixes may stop
/// at an active target only when its parameter namespace is preserved. Target
/// parameters substitute through authored arguments like call-frame
/// instantiation. Value-position call writes are collected before the jump.
pub(crate) fn summarize_transition_target_written_paths(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source_state: &State,
    target: typed_trees::statement::TransitionTargetHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    source_locals: &[String],
    require_complete: bool,
    origins: &CallOriginContext<'_>,
    machine_symbols: &MachineSymbols<'_>,
) -> Option<Vec<String>> {
    if !target.is_valid() {
        return Some(Vec::new());
    }
    if super::terminal_calls::target(program, machine, target).is_valid() {
        return super::terminal_calls::written_paths(
            program,
            machine,
            target,
            machine_symbols,
            symbols,
            inference,
            origins,
        );
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Terminal => Some(Vec::new()),
        TransitionTargetNode::Value(_) => Some(Vec::new()),
        TransitionTargetNode::Named { arguments, .. } => {
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_state =
                named_transition_target_state(program, machine, source_state, target)?;
            if inference.active_states.contains(&target_state.symbol) {
                // Named topology can be acyclic while ordinary calls return
                // to an active named state. Only a contextual prefix may
                // truncate that edge; a reusable summary must cover the body.
                if require_complete {
                    return None;
                }
                return named_transition_preserves_state_namespace(
                    program,
                    source_state,
                    target_state,
                    arguments,
                )
                .then(Vec::new);
            }
            inference.active_states.push(target_state.symbol);
            let summarize = if require_complete {
                summarize_complete_state_written_paths
            } else {
                summarize_state_written_paths
            };
            let target_writes = summarize(program, machine, target_state, symbols, inference);
            inference.active_states.pop();
            let target_writes = target_writes?;
            let parameters = program.state_parameters(target_state);
            // Named-state transfer substitutes the same finite candidate sets
            // call frames already carry: an actual bound to a divergent
            // referent set instantiates each proven route into the source
            // namespace rather than sinking the whole frame. An unproven
            // divergent actual still fails closed.
            let argument_types = parameters
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>();
            let argument_origins = origins.argument_origins(
                program,
                machine,
                machine_symbols,
                symbols,
                inference,
                arguments,
                &argument_types,
            )?;
            let mut instantiated = Vec::new();
            for relative in target_writes {
                for path in instantiate_written_path_for_origins(
                    program,
                    machine,
                    &relative,
                    Some(&FramePlaceOrigin {
                        path: "self".to_owned(),
                        precision: FramePathPrecision::Exact,
                        source: Default::default(),
                    }),
                    parameters,
                    arguments,
                    source_locals,
                    symbols,
                    inference,
                    Some(&argument_origins),
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
