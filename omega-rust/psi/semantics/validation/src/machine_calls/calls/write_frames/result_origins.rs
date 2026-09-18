//! Body-derived reference leaves in a direct or owned helper result. A return type
//! supplies structure, never a caller storage origin or permission to borrow it.
//!
//! Ordinary value arms are one state's own result routes; ordinary named edges
//! compose the target state's relation through the authored arguments exactly
//! like call-frame instantiation. The reachable named subgraph must be finite:
//! a cycle keeps the result relation explicitly opaque instead of solving
//! leaves against a truncated depth-first walk.

use super::path_instantiation::aggregate_arguments::{
    AggregateOrigins, AggregateResolver, ReferenceLeaf, ReferenceResolver,
    reference_leaves_with_origins,
};
use super::stored_origins::{StoredLocalOrigins, canonical_reference_origins};
use super::{
    ExpressionHandle, FramePathPrecision, FramePlaceOrigin, Machine, StatementNode,
    TableCallExpression, TopLevelSymbols, TypeReferenceHandle, TypedTrees, machine_state_by_symbol,
};
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::state_write_walk::{
    StateWriteQuery, walk_state_write_prefix,
};
use crate::machine_calls::calls::write_frames::transition_topology::{
    named_state_transition_subgraph_is_acyclic, named_transition_target_state,
};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{TransitionExit, TransitionTargetHandle, TransitionTargetNode};

mod input_moves;
mod input_sources;

pub(super) fn call_result_origins(
    program: &TypedTrees,
    caller_machine: &Machine,
    call: &TableCallExpression,
    expected: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    resolve_reference: &ReferenceResolver<'_>,
    resolve_origins: &AggregateResolver<'_>,
) -> Option<AggregateOrigins> {
    // A stale or missing selected target cannot be repaired by its spelling.
    let (machine, state) = machine_state_by_symbol(program, call.target_symbol)?;
    let parameters = program.state_parameters(state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if include_shared
        && arguments
            .iter()
            .copied()
            .chain(call.receiver.is_valid().then_some(call.receiver))
            .any(|expression| {
                super::local_aliases::expression_has_exclusive_borrow(
                    program,
                    expression,
                    &|target| {
                        let Some((state, _, _)) = super::caller_aliases::caller_statement_at_site(
                            program,
                            caller_machine,
                            super::caller_aliases::CallerWriteSite::Expression(target),
                        ) else {
                            return true;
                        };
                        crate::value_custody::places::declared_place_type_raw(
                            program,
                            caller_machine,
                            Some(state),
                            target,
                        )
                        .is_none_or(|reference| {
                            super::type_reference_is_reference(program, reference)
                        })
                    },
                )
            })
    {
        // A proven returned leaf cannot excuse exposure in another operand.
        return None;
    }
    if inference.active_states.contains(&state.symbol)
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
        || !program.machine_type_parameters(machine).is_empty()
        || !call.machine_arguments.is_empty()
        || call.receiver.is_valid() != machine.attached_data.is_some()
        || arguments.len()
            != parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        || !super::isolation::aggregate_storage_types_match(program, state.return_type, expected)
    {
        return None;
    }
    // Named return routes compose only while the reachable named subgraph is
    // finite. A cycle may re-enter an active ancestor mid-proof, where the
    // result relation would depend on the recursion stack that produced it.
    if !named_state_transition_subgraph_is_acyclic(program, machine, state) {
        return None;
    }
    let mut relative = state_result_origins(
        program,
        machine,
        state,
        state.return_type,
        symbols,
        inference,
        include_shared,
        &mut Vec::new(),
    )?;
    instantiate_result_origins(
        program,
        caller_machine,
        machine,
        parameters,
        call.receiver.is_valid().then_some(call.receiver),
        arguments,
        &mut relative,
        symbols,
        inference,
        include_shared,
        resolve_reference,
        resolve_origins,
    )
}

/// One state's contribution to the callee's result relation, expressed in that
/// state's own parameter namespace. Every ordinary route out of the state
/// contributes: a trailing expression, each value arm, and each named edge
/// through its target state's relation. `expected_result` is the called
/// machine's declared result type: named states may omit their own `-> T`
/// annotation, so every reachable route resolves its leaves against that one
/// boundary type. `complete` memoizes computed states so a shared acyclic
/// target is not re-derived for every incoming edge.
#[allow(clippy::too_many_arguments)]
fn state_result_origins<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    expected_result: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    complete: &mut Vec<(SymbolHandle, AggregateOrigins)>,
) -> Option<AggregateOrigins> {
    if let Some((_, origins)) = complete.iter().find(|(symbol, _)| *symbol == state.symbol) {
        return Some(origins.clone());
    }
    // A still-active ancestor is a named cycle this query must not reuse; the
    // acyclicity gate normally rejects it first, but the guard also covers a
    // `self`-spelled named edge that returns to this same state.
    if inference.active_states.contains(&state.symbol) {
        return None;
    }
    input_moves::validate_frozen_inputs(program, machine, state, include_shared)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let parameters = program.state_parameters(state);
    // The shared transfer freezes local aliases and carrier leaves. Incoming
    // reference bindings also anchor this exported relation and cannot be
    // exposed for replacement anywhere in the helper, including a terminal
    // sibling expression. Content writes through their owned suffixes remain
    // ordinary producer effects.
    if statements.iter().any(|statement| {
        super::statement_value_expression_roots(program, statement)
            .into_iter()
            .any(|expression| {
                (!include_shared
                    && super::local_aliases::expression_reborrows_stable_alias_binding(
                        program,
                        expression,
                        parameters,
                        &[],
                    ))
                    || super::local_aliases::expression_has_exclusive_borrow(
                        program,
                        expression,
                        &|target| {
                            super::reference_origins::declared_origin_root(program, machine, target)
                                .is_none()
                        },
                    )
            })
    }) {
        return None;
    }
    for statement in statements {
        let source = match statement {
            StatementNode::LocalData(local)
                if super::type_reference_is_reference(program, local.type_reference) =>
            {
                local.initial_value
            }
            StatementNode::Assignment(assignment)
                if super::expression_may_rebind_mutable_alias(
                    program,
                    machine,
                    state,
                    assignment.value,
                ) =>
            {
                assignment.value
            }
            _ => continue,
        };
        if super::frame_place_path(program, source).is_some() {
            let source = match program.expression_table.expression(source) {
                super::ExpressionNode::Borrow(borrow) => borrow.target,
                _ => source,
            };
            super::reference_origins::declared_origin_root(program, machine, source)?;
        }
    }
    inference.active_states.push(state.symbol);
    let result = inference.with_local_scope(|inference| {
        // The whole body still has to satisfy the shared-transfer fences a
        // direct helper result enforced: producer writes, rebinding checks,
        // and operand exposure in every statement, including transition
        // arguments and terminal result expressions, are validated by one
        // complete walk before any arm contributes leaves.
        let context = walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            inference,
            &mut Vec::new(),
            if include_shared {
                Some(StateWriteQuery::ReferenceResult)
            } else {
                None
            },
        )?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let mut relative = AggregateOrigins::default();
        for (index, statement) in statements.iter().enumerate() {
            match statement {
                // A non-final expression is a discarded statement and never a
                // result route.
                StatementNode::Expression(result) if index + 1 == statements.len() => {
                    merge_result_arm(
                        program,
                        machine,
                        state,
                        statement,
                        index,
                        *result,
                        expected_result,
                        &mut relative,
                        symbols,
                        inference,
                        include_shared,
                    )?;
                }
                // A crash route never produces an ordinary result.
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    for edge in [transition.target, transition.continuation] {
                        if !edge.is_valid() {
                            continue;
                        }
                        match program.statement_table.transition_target(edge) {
                            TransitionTargetNode::Value(result) => merge_result_arm(
                                program,
                                machine,
                                state,
                                statement,
                                index,
                                *result,
                                expected_result,
                                &mut relative,
                                symbols,
                                inference,
                                include_shared,
                            )?,
                            TransitionTargetNode::Named { arguments, .. } => merge_named_edge(
                                program,
                                machine,
                                state,
                                statement,
                                index,
                                edge,
                                *arguments,
                                expected_result,
                                &mut relative,
                                symbols,
                                inference,
                                include_shared,
                                complete,
                            )?,
                            // A bare `-> self` re-enters this exact state with
                            // the same parameter namespace; its result set is
                            // already this relation. Terminal arms return
                            // nothing.
                            TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Some(relative)
    });
    inference.active_states.pop();
    let relative = result?;
    complete.push((state.symbol, relative.clone()));
    Some(relative)
}

/// Evaluate one result expression against the prefix context immediately before
/// its own statement, then merge the canonical origins into the relation.
#[allow(clippy::too_many_arguments)]
fn merge_result_arm(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    boundary: &StatementNode,
    boundary_index: usize,
    result: ExpressionHandle,
    expected_result: TypeReferenceHandle,
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<()> {
    inference.with_local_scope(|inference| {
        let context = walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            inference,
            &mut Vec::new(),
            Some(if include_shared {
                StateWriteQuery::ReferenceBefore(boundary)
            } else {
                StateWriteQuery::Before(boundary)
            }),
        )?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let returned = reference_leaves_with_origins(
            program,
            machine,
            result,
            expected_result,
            "",
            symbols,
            inference,
            include_shared,
            &|expression, reference, implicit_borrow, inference| {
                if include_shared {
                    super::reference_subjects::value_origin(
                        program,
                        machine,
                        expression,
                        symbols,
                        inference,
                        &context.aliases,
                        &context.stored,
                        implicit_borrow,
                    )
                    .or_else(|| {
                        super::reference_subjects::unknown_readonly_origin(program, reference, "")
                    })
                    .map(|origin| vec![origin])
                } else {
                    super::reference_origins::exclusive_reference_origins(
                        program, machine, expression, symbols, inference,
                    )
                }
            },
            &|expression, reference, _| {
                super::stored_origins::reference_leaves_before_statement_for_query(
                    program,
                    state,
                    boundary,
                    expression,
                    reference,
                    Some(&context.stored),
                    None,
                    include_shared,
                )
            },
        )?;
        merge_relative_origins(
            program,
            machine,
            state,
            boundary_index,
            returned,
            &context.aliases,
            &context.stored,
            relative,
            include_shared,
        )
    })
}

/// Merge one batch of origins expressed in `state`'s own namespace into the
/// relation. Leaves are canonicalized through the same prefix evidence the
/// producing route was evaluated in, so a local alias or stored carrier
/// resolves to its proven parameter root before the caller instantiates it.
#[allow(clippy::too_many_arguments)]
fn merge_relative_origins(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    boundary_index: usize,
    returned: AggregateOrigins,
    aliases: &[(String, FramePlaceOrigin)],
    stored: &[StoredLocalOrigins],
    relative: &mut AggregateOrigins,
    include_shared: bool,
) -> Option<()> {
    let parameters = program.state_parameters(state);
    for case in returned.cases {
        if !relative.cases.contains(&case) {
            relative.cases.push(case);
        }
    }
    for moved in returned.moves {
        if !relative.moves.iter().any(|existing| {
            existing.local_segments == moved.local_segments
                && existing.source == moved.source
                && existing.type_reference == moved.type_reference
        }) {
            relative.moves.push(moved);
        }
    }
    for leaf in returned.references {
        for origin in canonical_reference_origins(program, &leaf.origin, aliases, stored) {
            if include_shared
                && parameters.iter().any(|parameter| {
                    super::type_reference_is_reference(program, parameter.type_reference)
                        && (parameter.symbol == origin.source.root
                            || (parameter.is_self && machine.symbol == origin.source.root))
                })
            {
                // Check a borrowed carrier's loaded boundary while its
                // complete helper-prefix evidence is available. Caller
                // substitution cannot reconstruct a callee's frozen slots
                // or establish which payload the helper selected.
                super::reference_subjects::validate_source_projection(
                    program,
                    machine,
                    state,
                    boundary_index,
                    &origin.source,
                    stored,
                )?;
            }
            let leaf = ReferenceLeaf {
                local_suffix: leaf.local_suffix.clone(),
                local_segments: leaf.local_segments.clone(),
                origin,
            };
            if !relative.references.iter().any(|existing| {
                existing.local_suffix == leaf.local_suffix
                    && existing.local_segments == leaf.local_segments
                    && existing.origin.path == leaf.origin.path
                    && existing.origin.precision == leaf.origin.precision
                    && existing.origin.source == leaf.origin.source
            }) {
                relative.references.push(leaf);
            }
        }
    }
    Some(())
}

/// A named edge routes the call's result through the target state's own
/// relation. Instantiating the target relation through the authored arguments
/// is the same substitution a call frame performs; the resulting leaves land
/// in the source state's namespace and are canonicalized against the prefix
/// immediately before the transition.
#[allow(clippy::too_many_arguments)]
fn merge_named_edge(
    program: &TypedTrees,
    machine: &Machine,
    source_state: &State,
    boundary: &StatementNode,
    boundary_index: usize,
    edge: TransitionTargetHandle,
    edge_arguments: arena::HandleSpan<ExpressionHandle>,
    expected_result: TypeReferenceHandle,
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    complete: &mut Vec<(SymbolHandle, AggregateOrigins)>,
) -> Option<()> {
    let target_state = named_transition_target_state(program, machine, source_state, edge)?;
    // A named state may omit its own `-> T` annotation; one that declares it
    // must agree with the called machine's result type before its routes can
    // carry this call's relation.
    if target_state.return_type.is_valid()
        && !super::isolation::aggregate_storage_types_match(
            program,
            target_state.return_type,
            expected_result,
        )
    {
        return None;
    }
    let mut target_relative = state_result_origins(
        program,
        machine,
        target_state,
        expected_result,
        symbols,
        inference,
        include_shared,
        complete,
    )?;
    inference.with_local_scope(|inference| {
        let context = walk_state_write_prefix(
            program,
            machine,
            source_state,
            symbols,
            inference,
            &mut Vec::new(),
            Some(if include_shared {
                StateWriteQuery::ReferenceBefore(boundary)
            } else {
                StateWriteQuery::Before(boundary)
            }),
        )?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let arguments = program.statement_table.expression_handles(edge_arguments);
        let instantiated = instantiate_result_origins(
            program,
            machine,
            machine,
            program.state_parameters(target_state),
            None,
            arguments,
            &mut target_relative,
            symbols,
            inference,
            include_shared,
            &|actual, reference, implicit_borrow, inference| {
                if include_shared {
                    super::reference_subjects::value_origin(
                        program,
                        machine,
                        actual,
                        symbols,
                        inference,
                        &context.aliases,
                        &context.stored,
                        implicit_borrow,
                    )
                    .or_else(|| {
                        super::reference_subjects::unknown_readonly_origin(program, reference, "")
                    })
                    .map(|origin| vec![origin])
                } else {
                    super::reference_origins::exclusive_reference_origins(
                        program, machine, actual, symbols, inference,
                    )
                }
            },
            &|actual, reference, inference| {
                if include_shared {
                    super::stored_origins::reference_leaves_before_statement_for_query(
                        program,
                        source_state,
                        boundary,
                        actual,
                        reference,
                        Some(&context.stored),
                        None,
                        true,
                    )
                } else {
                    super::stored_origins::symbolic_reference_leaves(
                        program, machine, actual, reference, inference,
                    )
                }
            },
        )?;
        merge_relative_origins(
            program,
            machine,
            source_state,
            boundary_index,
            instantiated,
            &context.aliases,
            &context.stored,
            relative,
            include_shared,
        )
    })
}

/// Substitute a state's own-namespace result relation through actual argument
/// expressions. This is the shared tail of both the public call boundary and
/// an intra-machine named edge: a `None` receiver means the edge's `self`
/// parameter still spells the shared machine namespace, so such leaves are
/// already instantiated.
#[allow(clippy::too_many_arguments)]
fn instantiate_result_origins(
    program: &TypedTrees,
    host_machine: &Machine,
    callee_machine: &Machine,
    parameters: &[StateParameter],
    receiver: Option<ExpressionHandle>,
    arguments: &[ExpressionHandle],
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    resolve_reference: &ReferenceResolver<'_>,
    resolve_origins: &AggregateResolver<'_>,
) -> Option<AggregateOrigins> {
    // Body recursion and finite repeated calls in caller syntax are distinct.
    // Finish the guarded body proof before substituting caller expressions;
    // any enclosing body guards remain active during that substitution.
    let mut returned = input_moves::instantiate_moves(
        program,
        host_machine,
        parameters,
        arguments,
        relative,
        symbols,
        inference,
        include_shared,
        resolve_reference,
        resolve_origins,
    )?;
    returned.cases.extend(relative.cases.iter().cloned());
    for leaf in relative.references.iter() {
        if include_shared
            && !leaf.origin.source.root.is_valid()
            && leaf.origin.precision == FramePathPrecision::CollectionCoarse
        {
            // A shared unknown is still a leaf, not an empty result footprint.
            // Mutable unknowns cannot be issued by the reference resolver.
            returned.references.push(leaf.clone());
            continue;
        }
        let parameter = parameters.iter().find(|parameter| {
            leaf.origin.source.root == parameter.symbol
                || (parameter.is_self && leaf.origin.source.root == callee_machine.symbol)
        })?;
        let actual = if parameter.is_self {
            match receiver {
                Some(receiver) => receiver,
                // An intra-machine named edge has no receiver operand: `self`
                // is the same machine, so the leaf already names it.
                None => {
                    returned.references.push(leaf.clone());
                    continue;
                }
            }
        } else {
            let index = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|candidate| candidate.symbol == parameter.symbol)?;
            *arguments.get(index)?
        };
        for origin in input_sources::instantiate_source(
            program,
            host_machine,
            callee_machine,
            parameter,
            actual,
            &leaf.origin,
            symbols,
            inference,
            include_shared,
            resolve_reference,
            resolve_origins,
        )? {
            returned.references.push(ReferenceLeaf {
                local_suffix: leaf.local_suffix.clone(),
                local_segments: leaf.local_segments.clone(),
                origin,
            });
        }
    }
    Some(returned)
}
