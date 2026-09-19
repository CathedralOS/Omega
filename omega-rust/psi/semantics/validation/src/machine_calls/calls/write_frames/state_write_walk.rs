//! Walking state write prefixes to summarize the paths a state writes.

#[cfg(test)]
use super::PREFIX_WALKS;
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::alias_bindings::{
    rebind_stable_local_mutable_alias_origin, stable_local_mutable_alias_rebinding_is_representable,
};
use crate::machine_calls::calls::write_frames::alias_origins::{
    stable_alias_initializer_origin, stable_alias_initializer_origins,
    stable_assignment_target_path, stable_local_reference_alias_origin,
    stable_local_reference_alias_origins,
};
use crate::machine_calls::calls::write_frames::assignment_targets::expression_is_effectful_indexed_place;
use crate::machine_calls::calls::write_frames::boundary_calls::{
    known_boundary_call_written_paths_for_parts, known_requirement_call_written_paths_for_parts,
};
use crate::machine_calls::calls::write_frames::caller_aliases::AssignmentWriteTarget;
use crate::machine_calls::calls::write_frames::caller_aliases::CallOriginContext;
use crate::machine_calls::calls::write_frames::caller_aliases::CallerWriteSite;
use crate::machine_calls::calls::write_frames::demand::{
    collect_expression_call_written_paths, statement_value_expression_roots,
    syntactic_call_written_paths,
};
use crate::machine_calls::calls::write_frames::inference::FrameInference;
use crate::machine_calls::calls::write_frames::isolation::type_is_caller_isolated_local;
use crate::machine_calls::calls::write_frames::known_call_written_paths_for_parts_with_origins;
use crate::machine_calls::calls::write_frames::local_aliases::expression_reborrows_unresolved_reference_binding;
use crate::machine_calls::calls::write_frames::permuted_cycle_frames::{
    summarize_state_written_paths_with_permuted_cycles, summarize_transition_target_written_paths,
};
use crate::machine_calls::calls::write_frames::place_paths::{
    FramePathPrecision, FramePlaceOrigin, append_place_suffix, coarse_place_path, split_place_root,
};
use crate::machine_calls::calls::write_frames::state_paths::{
    push_visible_frame_path, relative_state_path_is_visible,
};
use crate::machine_calls::calls::write_frames::stored_origins::{
    StoredLocalOrigins, expand_write_path,
};
use crate::machine_calls::calls::write_frames::transition_topology::{
    named_state_transition_subgraph_is_acyclic, named_transition_subgraph_is_acyclic,
};
use crate::machine_calls::calls::write_frames::type_capabilities::{
    type_may_carry_write, type_reference_is_reference,
};
use crate::machine_calls::calls::write_frames::{
    alias_bindings, local_aliases, reference_subjects, stored_origins, wire_codecs,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

pub(crate) fn summarize_state_written_paths(
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
    // A state that reaches a named cycle cannot reuse a depth-first prefix as
    // its summary: the walk truncates back-edges against whichever ancestors
    // happen to be active, so the result belongs to that recursion stack
    // alone. Solve the permuted-cycle equations for the honest transitive
    // frame instead. When they decline (a cycle rebinds a write-capable
    // parameter, or a nested call needs this stack's evidence) the DFS prefix
    // still answers this query but must not be retained for other contexts.
    if !named_state_transition_subgraph_is_acyclic(program, machine, state) {
        if let Some(writes) = summarize_state_written_paths_with_permuted_cycles(
            program,
            machine,
            state,
            symbols,
            inference,
            complete_state_summaries,
        ) {
            // A solved fixpoint contains no truncation against this recursion
            // stack: a nested call blocked by an active state fails the whole
            // solve instead of producing a partial frame, so a successful
            // result is safe to retain for later queries.
            complete_state_summaries.push((state.symbol, writes.clone()));
            return Some(writes);
        }
        return walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            inference,
            complete_state_summaries,
            None,
        )
        .map(|prefix| prefix.written);
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

pub(crate) struct StateWritePrefix {
    written: Vec<String>,
    pub(crate) aliases: Vec<(String, FramePlaceOrigin)>,
    /// Exclusive-reference locals whose proven referents form a divergent
    /// candidate set rather than one origin. Call substitution consumes the
    /// complete set; no raw local name may replace it at a write boundary.
    pub(crate) divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    pub(crate) stored: Vec<StoredLocalOrigins>,
    pub(crate) assignment: Option<AssignmentWriteTarget>,
}

pub(crate) enum StateWriteQuery<'statement> {
    Before(&'statement StatementNode),
    ReferenceBefore(&'statement StatementNode),
    ReferenceResult,
    Assignment(&'statement StatementNode),
}

/// The same state transfer computes whole-body summaries and the alias context
/// immediately before a demand query. Prefix results never enter the complete
/// state-summary cache.
pub(crate) fn walk_state_write_prefix(
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
    if crate::declarations::standard_declarations::is_core_vector_surface_state(
        program, machine, state,
    ) {
        return None;
    }
    #[cfg(test)]
    PREFIX_WALKS.with(|walks| walks.set(walks.get() + 1));
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    // Exclusive-reference locals bound to a divergent conditional result keep
    // their whole proven referent set here rather than a single origin. A
    // write through such a binding lands on one of its candidates, so the
    // frame unions every route, including through nested call substitution.
    let mut divergent_alias_origins = Vec::<(String, Vec<FramePlaceOrigin>)>::new();
    let include_shared = matches!(
        query,
        Some(StateWriteQuery::ReferenceBefore(_) | StateWriteQuery::ReferenceResult)
    );
    let mut stored = if include_shared {
        parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .filter_map(|parameter| {
                let reference =
                    reference_subjects::carrier_storage_type(program, parameter.type_reference)?;
                let origins = stored_origins::declared_origins_for_query(
                    program,
                    parameter.symbol,
                    parameter.name.as_str(),
                    reference,
                    true,
                )?;
                (!type_reference_is_reference(program, parameter.type_reference)
                    || !origins.references.is_empty())
                .then_some(origins)
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut written = Vec::new();

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    let statements = program.statement_table.statements(state.statement_nodes);
    for (statement_index, statement) in statements.iter().enumerate() {
        if matches!(query, Some(StateWriteQuery::Before(before) | StateWriteQuery::ReferenceBefore(before)) if std::ptr::eq(before, statement))
        {
            return Some(StateWritePrefix {
                written,
                aliases: local_alias_origins,
                divergent: divergent_alias_origins,
                stored,
                assignment: None,
            });
        }
        let queried_assignment = matches!(query,
            Some(StateWriteQuery::Assignment(candidate)) if std::ptr::eq(candidate, statement));
        if queried_assignment
            && local_alias_origins.is_empty()
            && stored.is_empty()
            && divergent_alias_origins.is_empty()
            && let StatementNode::Assignment(assignment) = statement
            && let Some(path) = coarse_place_path(program, assignment.target)
        {
            // The direct-store query has always admitted an untracked coarse
            // target without evaluating operand effects. Keep that boundary
            // when locating the target and its origins in one prefix walk.
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
            return Some(StateWritePrefix {
                written,
                aliases: local_alias_origins,
                divergent: divergent_alias_origins,
                stored,
                assignment: Some(AssignmentWriteTarget::Storage { paths: vec![path] }),
            });
        }
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
                    &divergent_alias_origins,
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
                                &divergent_alias_origins,
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
            program,
            machine,
            state,
            statement,
            &stored,
            &local_alias_origins,
        ) {
            return None;
        }
        if !divergent_alias_origins.is_empty() {
            let divergent_roots = divergent_alias_origins
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>();
            if local_aliases::statement_mentions_place_roots(program, statement, &divergent_roots) {
                // Assignment transfer distinguishes replacing the local
                // reference binding from writing through its referent set.
                if let StatementNode::Assignment(assignment) = statement
                    && let Some(relative) = coarse_place_path(program, assignment.target)
                    && let Some(position) = divergent_alias_origins
                        .iter()
                        .position(|(name, _)| name.as_str() == split_place_root(&relative).0)
                {
                    if local_aliases::expression_mentions_place_roots(
                        program,
                        assignment.value,
                        &divergent_roots,
                    ) {
                        return None;
                    }
                    let (_, suffix) = split_place_root(&relative);
                    // Only a bare-name target can rebind the binding itself;
                    // a member or indexed target writes inside the referent
                    // (`coarse_place_path` already coarsened `alias[i]` to
                    // `alias`, so it lands here as a write, not a rebind).
                    let bare_name_target = matches!(
                        program.expression_table.expression(assignment.target),
                        typed_trees::expression::ExpressionNode::Name(_)
                    );
                    if bare_name_target
                        && suffix.is_empty()
                        && local_aliases::expression_may_rebind_mutable_alias(
                            program,
                            machine,
                            state,
                            assignment.value,
                        )
                    {
                        let origins = stable_alias_initializer_origins(
                            program,
                            machine,
                            &machine_symbols,
                            inference,
                            assignment.value,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            &divergent_alias_origins,
                            symbols,
                            true,
                            &stored,
                        )?;
                        if queried_assignment {
                            return Some(StateWritePrefix {
                                written,
                                aliases: local_alias_origins,
                                divergent: divergent_alias_origins,
                                stored,
                                assignment: Some(AssignmentWriteTarget::LocalBindingReplacement {
                                    path: relative,
                                }),
                            });
                        }
                        divergent_alias_origins[position].1 = origins;
                    } else {
                        // A value RHS into a reference-typed interior slot
                        // writes through that slot's own referent, which the
                        // path frame cannot name.
                        if !bare_name_target
                            && !local_aliases::expression_may_rebind_mutable_alias(
                                program,
                                machine,
                                state,
                                assignment.value,
                            )
                            && crate::value_custody::places::declared_place_type_raw(
                                program,
                                machine,
                                Some(state),
                                assignment.target,
                            )
                            .is_some_and(|reference| {
                                type_reference_is_reference(program, reference)
                            })
                        {
                            return None;
                        }
                        // A bare write through the binding lands on one proven
                        // referent, so the frame records the whole union. Each
                        // candidate still expands through the ordinary write
                        // path so stored carrier slots contribute their
                        // overlapping external leaves.
                        let mut paths = Vec::new();
                        for origin in &divergent_alias_origins[position].1 {
                            let composed = match origin.precision {
                                FramePathPrecision::Exact => {
                                    append_place_suffix(&origin.path, suffix)
                                }
                                FramePathPrecision::CollectionCoarse => origin.path.clone(),
                            };
                            for path in expand_write_path(&composed, &local_alias_origins, &stored)
                            {
                                if !paths.contains(&path) {
                                    paths.push(path);
                                }
                            }
                        }
                        if queried_assignment {
                            return Some(StateWritePrefix {
                                written,
                                aliases: local_alias_origins,
                                divergent: divergent_alias_origins,
                                stored,
                                assignment: Some(AssignmentWriteTarget::Storage { paths }),
                            });
                        }
                        for path in paths {
                            push_visible_frame_path(&mut written, path, parameters, &locals)?;
                        }
                    }
                    continue;
                }
            }
        }
        let origins = CallOriginContext {
            parameters,
            isolated_locals: &isolated_local_roots,
            aliases: &local_alias_origins,
            divergent: &divergent_alias_origins,
            stored: &stored,
        };
        for expression in statement_value_expression_roots(program, statement) {
            local_aliases::validate_divergent_projections(
                program,
                machine,
                expression,
                &divergent_alias_origins,
            )?;
            let exposes_reference_binding = if include_shared {
                local_aliases::expression_reborrows_stable_alias_binding(
                    program,
                    expression,
                    parameters,
                    &local_alias_origins,
                )
            } else {
                expression_reborrows_unresolved_reference_binding(
                    program,
                    machine,
                    expression,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    &divergent_alias_origins,
                )
            };
            if exposes_reference_binding
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
                && !(include_shared
                    && declared_stored_origins.is_some()
                    && !reference_subjects::bindings::calls_expose_bindings(
                        program,
                        expression,
                        parameters,
                        &local_alias_origins,
                    ))
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
                complete_state_summaries,
                &origins,
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
            StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                let replaces_local_binding = representable_alias_rebinding
                    || (include_shared
                        && reference_subjects::replaces_binding(program, machine, statement)?);
                let replaces_stored_binding = (!replaces_local_binding
                    && (stored_origins::assignment_replaces_case_binding(
                        program,
                        assignment,
                        &stored,
                        &local_alias_origins,
                    ) || (include_shared
                        && stored_origins::assignment_replaces_reference_ancestor(
                            program,
                            assignment,
                            &stored,
                            &local_alias_origins,
                        ))))
                    || alias_bindings::assignment_replaces_untracked_reference(
                        program,
                        machine,
                        state,
                        assignment,
                        &local_alias_origins,
                    );
                // A carrier replacement retires the binding's stored evidence,
                // so the transfer installs the replacement's own proven rows
                // under the same root. An unproven source keeps the walk
                // opaque rather than reviving the overwritten rows.
                let replacement = if replaces_stored_binding {
                    Some(stored_origins::assigned_stored_origins(
                        program,
                        machine,
                        state,
                        statement,
                        assignment,
                        &local_alias_origins,
                        &stored,
                        symbols,
                        inference,
                        include_shared,
                    )?)
                } else {
                    None
                };
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
                                    &divergent_alias_origins,
                                    symbols,
                                    true,
                                    &stored,
                                )
                            };
                            origin.or_else(|| {
                                if !include_shared {
                                    return None;
                                }
                                let reference =
                                    crate::value_custody::places::declared_place_type_raw(
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
                            divergent: divergent_alias_origins,
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
                        divergent: divergent_alias_origins,
                        stored,
                        assignment: Some(AssignmentWriteTarget::Storage { paths }),
                    });
                }
                for path in paths {
                    push_visible_frame_path(&mut written, path, parameters, &locals)?;
                }
                // The write itself expanded through the pre-state evidence;
                // only later statements see the replacement's own origins.
                if let Some(replacement) = replacement {
                    inference.record_local(&replacement);
                    if let Some(existing) = stored
                        .iter_mut()
                        .find(|local| local.local_symbol == replacement.local_symbol)
                    {
                        *existing = replacement;
                    } else {
                        stored.push(replacement);
                    }
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
                if nested_receiver_members.first().is_some_and(|root| {
                    divergent_alias_origins.iter().any(|(name, _)| name == root)
                }) {
                    return None;
                }
                // A synthesized wire codec has no body to summarize and no
                // parameters to instantiate; its frame is its exclusively
                // borrowed arguments. The type-name receiver must never reach
                // the ownership floor, which would poison it as a place.
                let nested_writes = if wire_codecs::is_wire_codec_call(program, nested_call) {
                    // A synthesized codec frames its borrowed arguments by
                    // coarse place spelling; a divergent binding's name
                    // cannot stand in for its referent set, so it stays
                    // opaque rather than drop the write.
                    if arguments.iter().any(|argument| {
                        local_aliases::expression_mentions_place_roots(
                            program,
                            *argument,
                            &divergent_alias_origins
                                .iter()
                                .map(|(name, _)| name.clone())
                                .collect::<Vec<_>>(),
                        )
                    }) {
                        return None;
                    }
                    wire_codecs::known_wire_codec_call_written_paths(program, nested_call)
                } else {
                    let argument_types = super::call_targets::call_argument_types(
                        program,
                        machine,
                        nested_call.target_symbol,
                        nested_call.target.as_str(),
                        &nested_receiver_members,
                        CallerWriteSite::Call(nested_call),
                        &machine_symbols,
                        symbols,
                    );
                    let argument_origins = origins.argument_origins(
                        program,
                        machine,
                        &machine_symbols,
                        symbols,
                        inference,
                        arguments,
                        &argument_types,
                    )?;
                    let has_divergent_actual = arguments
                        .iter()
                        .any(|argument| origins.mentions_divergent(program, *argument));
                    known_call_written_paths_for_parts_with_origins(
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
                        (!arguments.iter().any(|argument| {
                            expression_is_effectful_indexed_place(program, *argument)
                        }))
                        .then(|| {
                            known_boundary_call_written_paths_for_parts(
                                program,
                                machine,
                                &machine_symbols,
                                symbols,
                                &nested_receiver_members,
                                nested_call.target.as_str(),
                                CallerWriteSite::Call(nested_call),
                                arguments,
                                inference,
                            )
                        })
                        .flatten()
                    })
                    .or_else(|| {
                        (!arguments.iter().any(|argument| {
                            expression_is_effectful_indexed_place(program, *argument)
                        }))
                        .then(|| {
                            known_requirement_call_written_paths_for_parts(
                                program,
                                machine,
                                &machine_symbols,
                                symbols,
                                &nested_receiver_members,
                                nested_call.target.as_str(),
                                None,
                                CallerWriteSite::Call(nested_call),
                                arguments,
                                inference,
                            )
                        })
                        .flatten()
                    })
                    .or_else(|| {
                        if has_divergent_actual {
                            return None;
                        }
                        syntactic_call_written_paths(
                            program,
                            &nested_receiver_members,
                            arguments,
                            &machine_symbols,
                            symbols,
                        )
                    })
                }?;
                for relative in origins
                    .close_paths(nested_writes)
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
                    if target.is_valid()
                        && let TransitionTargetNode::Named { arguments, .. } =
                            program.statement_table.transition_target(target)
                        && program
                            .statement_table
                            .expression_handles(*arguments)
                            .iter()
                            .any(|argument| origins.mentions_divergent(program, *argument))
                    {
                        // Named-state transfer does not yet substitute finite
                        // candidate sets. It must not drop the raw local as
                        // private storage after instantiating target writes.
                        return None;
                    }
                    if (!local_alias_origins.is_empty()
                        || !stored.is_empty()
                        || !divergent_alias_origins.is_empty())
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
                    } else if let Some(origins) = declared_stored_origins {
                        inference.record_local(&origins);
                        stored.push(origins);
                    } else if let Some(origins) = (!include_shared)
                        .then(|| {
                            stable_local_reference_alias_origins(
                                program,
                                machine,
                                &machine_symbols,
                                inference,
                                local,
                                parameters,
                                &isolated_local_roots,
                                &local_alias_origins,
                                &divergent_alias_origins,
                                symbols,
                                &stored,
                            )
                        })
                        .flatten()
                    {
                        // A divergent exclusive-reference binding keeps its
                        // whole proven referent set for later assignment and
                        // call substitution, including copies of this binding.
                        divergent_alias_origins.push((local.name.as_str().to_owned(), origins));
                    } else {
                        // A binding the transfer cannot name stays opaque —
                        // unless every later mention re-exports it intact
                        // through a pure tail return, which transports its
                        // referent set wholesale while the result relation
                        // resolves it through this same initializer. Any
                        // other mention — a write through it, a reborrow, or
                        // a transport into another binding — loses the set.
                        let mut reexported = false;
                        for later in &statements[statement_index + 1..] {
                            if !local_aliases::statement_mentions_place_roots(
                                program,
                                later,
                                &[local.name.as_str().to_owned()],
                            ) {
                                continue;
                            }
                            if !alias_bindings::statement_returns_reference_without_effects(
                                program, state, later,
                            ) {
                                return None;
                            }
                            reexported = true;
                        }
                        if !reexported {
                            return None;
                        }
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
                    && !divergent_alias_origins
                        .iter()
                        .any(|(name, _)| name == local.name.as_str())
                {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    (query.is_none() || matches!(query, Some(StateWriteQuery::ReferenceResult))).then_some(
        StateWritePrefix {
            written,
            aliases: local_alias_origins,
            divergent: divergent_alias_origins,
            stored,
            assignment: None,
        },
    )
}
