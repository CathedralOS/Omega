//! Body-derived reference leaves in a direct or owned helper result. A return type
//! supplies structure, never a caller storage origin or permission to borrow it.

use super::path_instantiation::aggregate_arguments::{
    AggregateOrigins, AggregateResolver, ReferenceLeaf, ReferenceResolver,
    reference_leaves_with_origins,
};
use super::stored_origins::canonical_reference_origins;
use super::{
    Machine, StatementNode, TableCallExpression, TopLevelSymbols, TypeReferenceHandle, TypedTrees,
    machine_state_by_symbol,
};
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::state_write_walk::walk_state_write_prefix;
use typed_trees::statement::{TransitionExit, TransitionTargetNode};

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
    let statements = program.statement_table.statements(state.statement_nodes);
    // The result region is the trailing run of ordinary value-return arms.
    // A guarded conditional chain returns one of several expressions, a
    // failed guard falls through to the next statement, and a plain terminal
    // expression is the single-arm case. Transitions append no locals or
    // stored origins, so every arm shares the prefix context; a non-final
    // expression is a discarded statement and never joins the region.
    let mut result_start = statements.len();
    let mut results = Vec::new();
    while let Some(statement) = result_start
        .checked_sub(1)
        .and_then(|index| statements.get(index))
    {
        let result = match statement {
            StatementNode::Expression(result) if result_start == statements.len() => *result,
            StatementNode::Transition(transition)
                if transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid() =>
            {
                let TransitionTargetNode::Value(result) =
                    program.statement_table.transition_target(transition.target)
                else {
                    break;
                };
                *result
            }
            _ => break,
        };
        results.push(result);
        result_start -= 1;
    }
    let (prefix, result_region) = statements.split_at(result_start);
    let result_boundary = result_region.first()?;
    // Named/alternate return routes need a result relation over their graph;
    // an ordinary write summary alone cannot select their returned value.
    if prefix
        .iter()
        .any(|statement| matches!(statement, StatementNode::Transition(_)))
    {
        return None;
    }
    input_moves::validate_frozen_inputs(program, machine, state, include_shared)?;
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
        // Include the terminal expression's producer writes and rebinding
        // fences. It cannot change the frozen local origins, so the resulting
        // context is also the context in which its leaves are constructed.
        let context = walk_state_write_prefix(
            program,
            machine,
            state,
            symbols,
            inference,
            &mut Vec::new(),
            include_shared.then_some(crate::machine_calls::calls::write_frames::state_write_walk::StateWriteQuery::ReferenceResult),
        )?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let mut relative = AggregateOrigins {
            references: Vec::new(),
            cases: Vec::new(),
            moves: Vec::new(),
        };
        for result in &results {
            let returned = reference_leaves_with_origins(
                program,
                machine,
                *result,
                state.return_type,
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
                    } else {
                        super::reference_origins::exclusive_reference_origin(
                            program, machine, expression, symbols, inference,
                        )
                    }
                },
                &|expression, reference, _| {
                    super::stored_origins::reference_leaves_before_statement_for_query(
                        program,
                        state,
                        result_boundary,
                        expression,
                        reference,
                        Some(&context.stored),
                        None,
                        include_shared,
                    )
                },
            )?;
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
                for origin in canonical_reference_origins(
                    program,
                    &leaf.origin,
                    &context.aliases,
                    &context.stored,
                ) {
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
                            statements.len(),
                            &origin.source,
                            &context.stored,
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
        }
        Some(relative)
    });
    inference.active_states.pop();
    // Body recursion and finite repeated calls in caller syntax are distinct.
    // Finish the guarded body proof before substituting caller expressions;
    // any enclosing body guards remain active during that substitution.
    let mut relative = result?;
    let mut returned = input_moves::instantiate_moves(
        program,
        caller_machine,
        state,
        call,
        &mut relative,
        symbols,
        inference,
        include_shared,
        resolve_reference,
        resolve_origins,
    )?;
    returned.cases.extend(relative.cases);
    for leaf in relative.references {
        if include_shared
            && !leaf.origin.source.root.is_valid()
            && leaf.origin.precision == super::FramePathPrecision::CollectionCoarse
        {
            // A shared unknown is still a leaf, not an empty result footprint.
            // Mutable unknowns cannot be issued by the reference resolver.
            returned.references.push(leaf);
            continue;
        }
        let parameter = parameters.iter().find(|parameter| {
            leaf.origin.source.root == parameter.symbol
                || (parameter.is_self && leaf.origin.source.root == machine.symbol)
        })?;
        let actual = if parameter.is_self {
            call.receiver
        } else {
            let index = parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .position(|candidate| candidate.symbol == parameter.symbol)?;
            *arguments.get(index)?
        };
        for origin in input_sources::instantiate_source(
            program,
            caller_machine,
            machine,
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
