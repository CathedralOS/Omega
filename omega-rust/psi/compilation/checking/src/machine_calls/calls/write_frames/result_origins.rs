//! Body-derived reference leaves in a direct or owned helper result. A return type
//! supplies structure, never a caller storage origin or permission to borrow it.
//!
//! Ordinary value arms are one state's own result routes; ordinary named edges
//! compose the selected state or called machine's relation through the authored
//! arguments exactly like call-frame instantiation. The reachable named subgraph
//! is solved as finite storage-root equations. Only actual normal returns seed
//! origins; an internal backedge neither fabricates a result nor hides an unknown.

use super::path_instantiation::aggregate_arguments::{
    AggregateOrigins, AggregateResolver, ReferenceLeaf, ReferenceResolver, reference_leaves,
};
use super::stored_origins::canonical_reference_origins;
use super::{
    ExpressionHandle, FramePathPrecision, FramePlaceOrigin, Machine, StatementNode,
    TableCallExpression, TopLevelSymbols, TypeReferenceHandle, machine_state_by_symbol,
};
use crate::machine_calls::calls::write_frames::FrameInference;
use crate::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, walk_state_write_prefix_collected,
};
use crate::machine_calls::calls::write_frames::transition_topology::named_transition_target_state;
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{TransitionExit, TransitionTargetHandle, TransitionTargetNode};

mod input_moves;
mod input_sources;
pub(super) use input_sources::instantiate_source;
mod state_equations;

pub(super) fn call_result_origins(
    program: &dyn crate::CheckingInput,
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
    // Operand exposure and writes belong to the chronological call transfer.
    // Reference-shaped operands include ordinary borrowed collection windows;
    // their type alone cannot decide whether a descriptor binding was exposed.
    // Instantiate the returned leaves against that same captured caller prefix.
    if inference.active_states.contains(&state.symbol)
        || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
        || !program.machine_type_parameters(machine).is_empty()
        || !call.machine_arguments.is_empty()
        || call.receiver.is_valid() != machine.attached_data.is_some()
        || arguments.len() != parameters.iter().count()
        || !super::isolation::aggregate_storage_types_match(program, state.return_type, expected)
    {
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
    )?;
    instantiate_result_origins(
        program,
        caller_machine,
        parameters,
        arguments,
        &mut relative,
        symbols,
        inference,
        include_shared,
        resolve_reference,
        resolve_origins,
    )
}

/// Complete result relation in the requested state's own parameter namespace.
/// Internal transitions share one finite solve; actual machine invocations keep
/// their active-body guard and cannot borrow provisional state summaries.
#[allow(clippy::too_many_arguments)]
pub(super) fn state_result_origins<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    expected_result: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<AggregateOrigins> {
    if inference.active_states.contains(&state.symbol) {
        return None;
    }
    inference.active_states.push(state.symbol);
    let result = state_equations::solve(
        program,
        machine,
        state,
        expected_result,
        symbols,
        inference,
        include_shared,
    );
    inference.active_states.pop();
    result
}

/// Collect one state's real return seeds and named-edge substitutions using the
/// same chronological prefix evidence for both acyclic and cyclic graphs.
#[allow(clippy::too_many_arguments)]
fn state_equation<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    expected_result: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<state_equations::Equation<'program>> {
    input_moves::validate_frozen_inputs(
        program,
        machine,
        state,
        include_shared,
        symbols,
        inference,
    )?;
    let statements = program.statement_table.statements(state.statement_nodes);
    // Every borrowed origin must retain its declaration. The shared transfer
    // below decides whether a reborrow exposes an unresolved binding or writes
    // through an established referent; a separate syntactic ban on parameter
    // reborrows would discard that distinction for returned carriers. Content
    // writes remain ordinary producer effects, including in result operands.
    if statements.iter().any(|statement| {
        super::statement_value_expression_roots(program, statement)
            .into_iter()
            .any(|expression| {
                super::local_aliases::expression_has_exclusive_borrow(
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
    let already_active = inference.active_states.contains(&state.symbol);
    if !already_active {
        inference.active_states.push(state.symbol);
    }
    let result = inference.with_local_scope(|inference| {
        // The whole body still has to satisfy the shared-transfer fences a
        // direct helper result enforced: producer writes, rebinding checks,
        // and operand exposure in every statement, including transition
        // arguments and terminal result expressions, are validated by one
        // complete walk before any arm contributes leaves. That walk also
        // collects only boundaries consumed by result arms. Each selected
        // snapshot retains the complete alias/stored closure at that point;
        // discarded statements still undergo the same validation walk.
        let (context, prefixes) = walk_state_write_prefix_collected(
            program,
            machine,
            state,
            symbols,
            inference,
            include_shared,
            &|statement_index| match &statements[statement_index] {
                StatementNode::Expression(_) => statement_index + 1 == statements.len(),
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    transition.targets().any(|edge| {
                        matches!(
                            program.statement_table.transition_target(edge),
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Named { .. }
                        )
                    })
                }
                _ => false,
            },
        )?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let mut relative = AggregateOrigins::default();
        let mut edges = Vec::new();
        let mut returns = false;
        let merge_result = |result: ExpressionHandle,
                            statement: &StatementNode,
                            index: usize,
                            returns: &mut bool,
                            relative: &mut AggregateOrigins,
                            inference: &mut FrameInference| {
            *returns = true;
            merge_result_arm(
                program,
                machine,
                state,
                statement,
                index,
                result,
                expected_result,
                relative,
                symbols,
                inference,
                include_shared,
                prefixes.get(index).and_then(Option::as_ref),
            )
        };
        for (index, statement) in statements.iter().enumerate() {
            match statement {
                // A non-final expression is a discarded statement and never a
                // result route.
                StatementNode::Expression(result) if index + 1 == statements.len() => {
                    merge_result(
                        *result,
                        statement,
                        index,
                        &mut returns,
                        &mut relative,
                        inference,
                    )?;
                }
                // A crash route never produces an ordinary result.
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    for edge in transition.targets() {
                        match program.statement_table.transition_target(edge) {
                            TransitionTargetNode::Value(result) => {
                                merge_result(
                                    *result,
                                    statement,
                                    index,
                                    &mut returns,
                                    &mut relative,
                                    inference,
                                )?;
                            }
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
                                &mut edges,
                                &mut returns,
                                prefixes.get(index).and_then(Option::as_ref),
                            )?,
                            // Terminal arms return nothing.
                            TransitionTargetNode::Terminal => {}
                        }
                    }
                }
                _ => {}
            }
        }
        Some(state_equations::Equation {
            state,
            direct: returns.then_some(relative),
            edges,
        })
    });
    if !already_active {
        inference.active_states.pop();
    }
    result
}

/// Evaluate one result expression against the prefix context immediately before
/// its own statement, then merge the canonical origins into the relation.
#[allow(clippy::too_many_arguments)]
fn merge_result_arm(
    program: &dyn crate::CheckingInput,
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
    boundary_prefix: Option<&CollectedStatementPrefix>,
) -> Option<()> {
    inference.with_local_scope(|inference| {
        let context = boundary_prefix?;
        for local in &context.stored {
            inference.record_local(local);
        }
        let returned = if super::type_reference_is_reference(program, expected_result) {
            // A whole reference exports provenance before its caller supplies
            // generic/Self bindings. The actual-reference resolver owns storage
            // eligibility; aggregate leaf extraction below still requires its
            // concrete referent boundary before exporting embedded references.
            reference_result_relation(
                program,
                machine,
                state,
                boundary_index,
                result,
                expected_result,
                symbols,
                inference,
            )?
        } else {
            reference_leaves(
                program,
                machine,
                result,
                expected_result,
                "",
                symbols,
                inference,
                include_shared,
                &|expression, reference, _implicit_borrow, inference| {
                    if include_shared {
                        // Returned provenance needs the complete backing set,
                        // not an exact predicate subject. Collection selectors
                        // retain typed origins without claiming value equality.
                        super::reference_subjects::backing_origins(
                            program,
                            machine,
                            expression,
                            symbols,
                            inference,
                            &context.aliases,
                            &context.stored,
                        )
                        .or_else(|| {
                            super::reference_subjects::unknown_readonly_origin(
                                program, reference, "",
                            )
                            .map(|origin| vec![origin])
                        })
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
            )?
        };
        merge_relative_origins(
            program,
            machine,
            state,
            boundary_index,
            returned,
            context,
            relative,
            include_shared,
        )
    })
}

/// Adapt the already-collected prefix to the existing parameter-relative
/// expression resolver. Local declaration symbols authenticate alias identity;
/// the prefix supplies the referents after all preceding writes and rebinds.
#[allow(clippy::too_many_arguments)]
pub(super) fn reference_result_relation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    boundary_index: usize,
    result: ExpressionHandle,
    reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<AggregateOrigins> {
    use super::parameter_aliases::ParameterRelativeFrameOrigin;

    let parameters = program.state_parameters(state);
    let mut aliases = Vec::new();
    for statement in program
        .statement_table
        .statements(state.statement_nodes)
        .get(..boundary_index)?
    {
        let Some(local) = statement.as_local_data() else {
            continue;
        };
        // A fresh borrow can name owned local storage as well as a stored
        // reference. Retain that exact home symbol here; the prefix join and
        // exit lifetime check still decide its backing and whether it escapes.
        let declared = FramePlaceOrigin {
            path: local.name.as_str().to_owned(),
            precision: FramePathPrecision::Exact,
            source: super::FrameSourcePlace {
                root: local.symbol,
                builtin_coordinates: true,
                ..Default::default()
            },
        };
        // Preserve the declaration until the result merge crosses the
        // captured alias exactly once. Resolving here would turn a captured
        // reference into a fresh read of a subsequently replaced carrier slot.
        aliases.push((
            local.name.as_str().to_owned(),
            local.symbol,
            vec![ParameterRelativeFrameOrigin {
                place: declared,
                parameter_symbol: SymbolHandle::invalid(),
            }],
        ));
    }
    let origins = super::parameter_relative_origins::parameter_relative_place_or_carrier_origins(
        program, machine, result, parameters, &aliases, symbols, inference,
    )?;
    let access = crate::reference_shell(program, reference)?.access;
    Some(AggregateOrigins {
        references: origins
            .into_iter()
            .map(|origin| ReferenceLeaf {
                kind: super::stored_origins::StoredOriginKind::Reference(access),
                local_suffix: String::new(),
                local_segments: Vec::new(),
                may_select: false,
                origin: Some(origin.place),
                lifetime_dependencies: Vec::new(),
            })
            .collect(),
        ..Default::default()
    })
}

/// Merge one batch of origins expressed in `state`'s own namespace into the
/// relation. Leaves are canonicalized through the same prefix evidence the
/// producing route was evaluated in, so a local alias or stored carrier
/// resolves to its proven parameter root before the caller instantiates it.
#[allow(clippy::too_many_arguments)]
fn merge_relative_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    boundary_index: usize,
    returned: AggregateOrigins,
    context: &CollectedStatementPrefix,
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
        for origin in canonical_result_reference_origins(
            program,
            state,
            boundary_index,
            &leaf.origin,
            context,
        ) {
            if let Some(origin) = &origin
                && include_shared
                && parameters.iter().any(|parameter| {
                    super::type_reference_is_reference(program, parameter.type_reference)
                        && (parameter.symbol == origin.source.root)
                })
            {
                // Check a borrowed carrier's loaded boundary while its
                // complete helper-prefix evidence is available. Caller
                // substitution cannot reconstruct a callee's frozen slots
                // or establish which payload the helper selected.
                super::reference_subjects::validate_backing_projection(
                    program,
                    machine,
                    state,
                    boundary_index,
                    &origin.source,
                    &context.stored,
                )?;
            }
            let leaf = ReferenceLeaf {
                kind: leaf.kind,
                local_suffix: leaf.local_suffix.clone(),
                local_segments: leaf.local_segments.clone(),
                may_select: leaf.may_select,
                origin,
                lifetime_dependencies: leaf.lifetime_dependencies.clone(),
            };
            if !relative.references.iter().any(|existing| {
                existing.kind == leaf.kind
                    && existing.local_suffix == leaf.local_suffix
                    && existing.local_segments == leaf.local_segments
                    && existing.may_select == leaf.may_select
                    && existing.origin == leaf.origin
                    && existing.lifetime_dependencies == leaf.lifetime_dependencies
            }) {
                relative.references.push(leaf);
            }
        }
    }
    Some(())
}

/// Each divergent alias candidate is an already-captured referent. Resolve
/// the authored local through that candidate once so the shared canonicalizer
/// retains the crossed-reference boundary while projecting any later fields.
fn canonical_result_reference_origins(
    program: &dyn crate::CheckingInput,
    state: &State,
    boundary_index: usize,
    origin: &Option<FramePlaceOrigin>,
    context: &CollectedStatementPrefix,
) -> Vec<Option<FramePlaceOrigin>> {
    if let Some(origin) = origin
        && origin.source.root.is_valid()
        && let Some(local) = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .take(boundary_index)
            .find_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == origin.source.root => {
                    Some(local)
                }
                _ => None,
            })
        && let Some((name, candidates)) = context
            .divergent
            .iter()
            .find(|(name, _)| local.name.as_str() == name)
    {
        if candidates.is_empty() {
            return vec![None];
        }
        return candidates
            .iter()
            .flat_map(|candidate| {
                canonical_reference_origins(
                    program,
                    &Some(origin.clone()),
                    &[(name.clone(), candidate.clone())],
                    &context.stored,
                )
            })
            .collect();
    }
    canonical_reference_origins(program, origin, &context.aliases, &context.stored)
}

/// A named edge routes the call's result through the target state's own
/// relation. Instantiating the target relation through the authored arguments
/// is the same substitution a call frame performs; the resulting leaves land
/// in the source state's namespace and are canonicalized against the prefix
/// immediately before the transition.
#[allow(clippy::too_many_arguments)]
fn merge_named_edge<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    source_state: &'program State,
    boundary: &StatementNode,
    boundary_index: usize,
    edge: TransitionTargetHandle,
    edge_arguments: arena::HandleSpan<ExpressionHandle>,
    expected_result: TypeReferenceHandle,
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
    edges: &mut Vec<state_equations::Edge<'program>>,
    returns: &mut bool,
    boundary_prefix: Option<&CollectedStatementPrefix>,
) -> Option<()> {
    let TransitionTargetNode::Named {
        static_machine_parameter,
        ..
    } = program.statement_table.transition_target(edge)
    else {
        return None;
    };
    if static_machine_parameter.is_valid() {
        return None;
    }
    let call_target = super::terminal_calls::target(program, machine, edge);
    let (target_machine, target_state) = if call_target.is_valid() {
        let (callee, state) = machine_state_by_symbol(program, call_target)?;
        // A terminal invocation uses the same body-derived origin authority as
        // an expression call. Requirement signatures and recursive bodies do
        // not provide a result storage relation by themselves.
        if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || !callee.body_is_present
            || !program.machine_type_parameters(callee).is_empty()
            || !super::isolation::aggregate_storage_types_match(
                program,
                state.return_type,
                expected_result,
            )
        {
            return None;
        }
        (callee, state)
    } else {
        (
            machine,
            named_transition_target_state(program, machine, source_state, edge)?,
        )
    };
    let arguments = program.statement_table.expression_handles(edge_arguments);
    let parameters = program.state_parameters(target_state);
    if arguments.len() != parameters.iter().count() {
        return None;
    }
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
    let context = boundary_prefix?;
    if !call_target.is_valid() {
        edges.push(state_equations::Edge {
            target: target_state,
            statement_index: boundary_index,
            arguments: edge_arguments,
            prefix: context.clone(),
        });
        return Some(());
    }
    let mut target_relative = state_result_origins(
        program,
        target_machine,
        target_state,
        expected_result,
        symbols,
        inference,
        include_shared,
    )?;
    *returns = true;
    instantiate_edge_origins(
        program,
        machine,
        source_state,
        target_state,
        boundary,
        boundary_index,
        arguments,
        &mut target_relative,
        context,
        relative,
        symbols,
        inference,
        include_shared,
    )
}

/// Substitute a retained target relation at its exact captured source boundary.
/// Equation solving and ordinary terminal calls use the identical operation.
#[allow(clippy::too_many_arguments)]
fn instantiate_edge_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    source_state: &State,
    target_state: &State,
    boundary: &StatementNode,
    boundary_index: usize,
    arguments: &[ExpressionHandle],
    target_relative: &mut AggregateOrigins,
    context: &CollectedStatementPrefix,
    relative: &mut AggregateOrigins,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
    include_shared: bool,
) -> Option<()> {
    let parameters = program.state_parameters(target_state);
    inference.with_local_scope(|inference| {
        for local in &context.stored {
            inference.record_local(local);
        }
        let instantiated = instantiate_result_origins(
            program,
            machine,
            parameters,
            arguments,
            target_relative,
            symbols,
            inference,
            include_shared,
            &|actual, reference, _implicit_borrow, inference| {
                if include_shared {
                    super::reference_subjects::backing_origins(
                        program,
                        machine,
                        actual,
                        symbols,
                        inference,
                        &context.aliases,
                        &context.stored,
                    )
                    .or_else(|| {
                        super::reference_subjects::unknown_readonly_origin(program, reference, "")
                            .map(|origin| vec![origin])
                    })
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
            context,
            relative,
            include_shared,
        )
    })
}

/// Substitute a state's own-namespace result relation through actual argument
/// expressions. This is the shared tail of both the public call boundary and
/// a named completion. Every returned origin must resolve to an explicit
/// input parameter before its source crosses that boundary.
#[allow(clippy::too_many_arguments)]
fn instantiate_result_origins(
    program: &dyn crate::CheckingInput,
    host_machine: &Machine,
    parameters: &[StateParameter],
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
        // A provider's unknown spatial origin does not make its lifetime
        // dependencies namespace-free. Substitute every debt before either
        // the unknown or spatial result branch leaves the callee.
        let mut lifetime_dependencies = Vec::new();
        for dependency in &leaf.lifetime_dependencies {
            if dependency.origin.source.is_static() {
                lifetime_dependencies.push(dependency.clone());
                continue;
            }
            let parameter_index = typed_trees::signature::state_parameter_position(
                parameters,
                dependency.origin.source.root,
            )?;
            for origin in input_sources::instantiate_source(
                program,
                host_machine,
                &parameters[parameter_index],
                *arguments.get(parameter_index)?,
                &dependency.origin,
                symbols,
                inference,
                include_shared,
                resolve_reference,
                resolve_origins,
            )? {
                let dependency = super::stored_calls::SlotSource {
                    carrier: origin.source.clone(),
                    origin,
                    lifetime_access: dependency.lifetime_access,
                };
                if !lifetime_dependencies.contains(&dependency) {
                    lifetime_dependencies.push(dependency);
                }
            }
        }
        let Some(origin) = &leaf.origin else {
            returned.references.push(ReferenceLeaf {
                lifetime_dependencies,
                ..leaf.clone()
            });
            continue;
        };
        if include_shared
            && !origin.source.root.is_valid()
            && origin.precision == FramePathPrecision::CollectionCoarse
        {
            // A shared unknown is still a leaf, not an empty result footprint.
            // Mutable unknowns cannot be issued by the reference resolver.
            returned.references.push(ReferenceLeaf {
                lifetime_dependencies,
                ..leaf.clone()
            });
            continue;
        }
        let parameter = parameters
            .iter()
            .find(|parameter| origin.source.root == parameter.symbol)?;
        let actual = {
            let index = parameters
                .iter()
                .position(|candidate| candidate.symbol == parameter.symbol)?;
            *arguments.get(index)?
        };
        for origin in input_sources::instantiate_source(
            program,
            host_machine,
            parameter,
            actual,
            origin,
            symbols,
            inference,
            include_shared,
            resolve_reference,
            resolve_origins,
        )? {
            returned.references.push(ReferenceLeaf {
                kind: leaf.kind,
                local_suffix: leaf.local_suffix.clone(),
                local_segments: leaf.local_segments.clone(),
                may_select: leaf.may_select,
                origin: Some(origin),
                lifetime_dependencies: lifetime_dependencies.clone(),
            });
        }
    }
    Some(returned)
}
