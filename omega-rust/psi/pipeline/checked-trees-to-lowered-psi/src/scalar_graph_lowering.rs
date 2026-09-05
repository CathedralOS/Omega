//! Scalar-graph preparation, validation, partial evaluation, and lowering.

use super::*;

mod branch_destinations;
mod computations;
mod initializers;
mod storage;

pub(super) fn checked_scalar_computation_call_targets(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<Vec<symbols::SymbolHandle>, LoweringError> {
    computations::call_targets(checked, machine)
}

fn merge_known_parameters<T: Copy + Eq>(
    current: &mut Option<Vec<Option<T>>>,
    incoming: Vec<Option<T>>,
) {
    if let Some(current) = current {
        assert_eq!(current.len(), incoming.len());
        for (current, incoming) in current.iter_mut().zip(incoming) {
            if *current != incoming {
                *current = None;
            }
        }
    } else {
        *current = Some(incoming);
    }
}

fn acyclic_topological_order(successors: &[Vec<usize>]) -> Vec<usize> {
    let mut indegree = vec![0_usize; successors.len()];
    for targets in successors {
        for target in targets {
            indegree[*target] = indegree[*target]
                .checked_add(1)
                .expect("source state count fits usize");
        }
    }
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, indegree)| (*indegree == 0).then_some(index))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(successors.len());
    while let Some(state) = ready.iter().next().copied() {
        ready.remove(&state);
        order.push(state);
        for target in &successors[state] {
            indegree[*target] = indegree[*target]
                .checked_sub(1)
                .expect("graph indegree is positive before traversal");
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    assert_eq!(order.len(), successors.len());
    order
}

fn evaluate_known_scalar_graph(states: &[LoweredScalarBranchState]) -> Option<KnownDirectScalar> {
    let successors = states
        .iter()
        .map(|state| match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            LoweredScalarBranchTerminator::Return { .. }
            | LoweredScalarBranchTerminator::Crash(_) => Vec::new(),
        })
        .collect::<Vec<_>>();
    let topological_order = acyclic_topological_order(&successors);
    let mut known_parameters = vec![None; states.len()];
    known_parameters[0] = Some(vec![None; states[0].parameter_types.len()]);
    let mut return_values = Vec::new();
    let mut reachable_crash = false;
    for state_index in topological_order {
        let Some(mut values) = known_parameters[state_index].clone() else {
            continue;
        };
        for binding in &states[state_index].bindings {
            let value = match binding {
                LoweredScalarBinding::Expression(expression) => {
                    evaluate_direct_expression(expression, &values)
                }
                LoweredScalarBinding::DirectCall(_) => None,
            };
            values.push(value);
        }
        let evaluate_arguments = |arguments: &[LoweredDirectExpression]| {
            arguments
                .iter()
                .map(|argument| evaluate_direct_expression(argument, &values))
                .collect::<Vec<_>>()
        };
        match &states[state_index].terminator {
            LoweredScalarBranchTerminator::Jump { target, arguments } => {
                merge_known_parameters(
                    &mut known_parameters[*target],
                    evaluate_arguments(arguments),
                );
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => match evaluate_compile_known_boolean_expression(condition, &values) {
                Some(true) => merge_known_parameters(
                    &mut known_parameters[*when_true_target],
                    evaluate_arguments(when_true_arguments),
                ),
                Some(false) => merge_known_parameters(
                    &mut known_parameters[*when_false_target],
                    evaluate_arguments(when_false_arguments),
                ),
                None => {
                    merge_known_parameters(
                        &mut known_parameters[*when_true_target],
                        evaluate_arguments(when_true_arguments),
                    );
                    merge_known_parameters(
                        &mut known_parameters[*when_false_target],
                        evaluate_arguments(when_false_arguments),
                    );
                }
            },
            LoweredScalarBranchTerminator::Return { expression } => {
                return_values.push(evaluate_direct_expression(expression, &values));
            }
            LoweredScalarBranchTerminator::Crash(_) => reachable_crash = true,
        }
    }

    if reachable_crash {
        return None;
    }
    let expected = return_values.first().copied().flatten()?;
    return_values
        .into_iter()
        .all(|value| value == Some(expected))
        .then_some(expected)
}

pub(super) fn lower_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<LoweredPsi, LoweringError> {
    let prepared = prepare_standalone_scalar_graph_machine(checked, machine, graph)?;
    let machine_ids = [(machine, machine_id(1))];
    let requirement_counts = [(machine, prepared.contract.requirement_count())];
    let mut lowered = build_scalar_graph_module(
        &prepared.states,
        prepared.result_type,
        prepared.contract,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &machine_ids,
        &requirement_counts,
    )?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

/// Lower one scalar realization whose contract/satisfaction is retained by an
/// enclosing target-owned admission rather than reconstructed as a closed
/// standalone scalar contract.
pub(super) fn lower_selected_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<LoweredPsi, LoweringError> {
    let prepared = prepare_embedded_scalar_graph_machine(checked, machine, graph)?;
    let machine_ids = [(machine, machine_id(1))];
    let requirement_counts = [(machine, prepared.contract.requirement_count())];
    let mut lowered = build_scalar_graph_module(
        &prepared.states,
        prepared.result_type,
        prepared.contract,
        prepared.crash_routes,
        prepared.identity_reshuffles,
        prepared.partition_compositions,
        machine_id(1),
        0,
        &machine_ids,
        &requirement_counts,
    )?;
    finalize_operation_proofs(&mut lowered)?;
    Ok(lowered)
}

pub(super) fn prepare_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        machine,
        graph,
        ScalarContractMode::ClosedRuntimeValue,
    )
}

fn prepare_standalone_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        machine,
        graph,
        ScalarContractMode::StandaloneProofOnlyFloatResult,
    )
}

/// Prepare an exact scalar body for inclusion beneath an attached Unit root.
/// The enclosing checked call retains the target contract identity; unlike the
/// standalone scalar lane, a parameter-relative contract need not reduce to
/// one closed literal when the caller consumes only the runtime value.
pub(super) fn prepare_embedded_scalar_graph_machine(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
) -> Result<PreparedScalarMachine, LoweringError> {
    prepare_scalar_graph_machine_with_contract_mode(
        checked,
        machine,
        graph,
        ScalarContractMode::EmbeddedByEnclosingCall,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarContractMode {
    ClosedRuntimeValue,
    StandaloneProofOnlyFloatResult,
    EmbeddedByEnclosingCall,
}

fn prepare_scalar_graph_machine_with_contract_mode(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    graph: &CheckedScalarMachineGraph,
    contract_mode: ScalarContractMode,
) -> Result<PreparedScalarMachine, LoweringError> {
    let states = &graph.states;
    let entry_state = states.first().ok_or(LoweringError::Unsupported(
        "checked scalar control plan must contain an entry state",
    ))?;
    let result_type = terminal_scalar_type(entry_state.result_type)?;
    let (identity_reshuffles, partition_compositions) =
        lower_content_evidence(checked, machine, entry_state.state)?;
    let return_sink = states
        .iter()
        .any(|state| match &state.terminator {
            CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                matches!(when_true, CheckedScalarBranchDestination::Return { .. })
                    || matches!(when_false, CheckedScalarBranchDestination::Return { .. })
            }
            CheckedScalarStateTerminator::Return { statement_ordinal } => checked
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .any(|(_, root)| {
                    root.state == state.state
                        && root.statement_ordinal == *statement_ordinal
                        && root.role == CheckedScalarExpressionRole::Return
                }),
            _ => false,
        })
        .then_some(states.len());
    let lowered_state_count = states.len() + usize::from(return_sink.is_some());
    let mut lowered_states = Vec::with_capacity(lowered_state_count);
    let mut computations = computations::Expansion::new(checked, machine, lowered_state_count);

    for state in states {
        if terminal_scalar_type(state.result_type)? != result_type {
            return unsupported("scalar graph state result types must match exactly");
        }
        let parameter_types = state
            .parameter_types
            .iter()
            .copied()
            .map(terminal_scalar_type)
            .collect::<Result<Vec<_>, _>>()?;
        let prepared = initializers::prepare(checked, machine, state, parameter_types)?;
        let value_types = &prepared.value_types;
        let scalar_bindings = &prepared.scalar_bindings;
        let terminator = match &state.terminator {
            CheckedScalarStateTerminator::Return { statement_ordinal } => {
                let computed_entry = if let Some(target) = return_sink {
                    computations.return_value(
                        state.state,
                        *statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                        scalar_bindings,
                        value_types,
                        result_type,
                        target,
                    )?
                } else {
                    None
                };
                if let Some(target) = computed_entry {
                    LoweredScalarBranchTerminator::Jump {
                        target,
                        arguments: computations::parameters(value_types),
                    }
                } else {
                    let expression = scalar_bindings.expression_at(
                        checked,
                        state.state,
                        *statement_ordinal,
                        CheckedScalarExpressionRole::Return,
                    )?;
                    if expression.scalar_type() != result_type {
                        return unsupported(
                            "checked scalar return type must match the machine result",
                        );
                    }
                    validate_direct_parameter_types(&expression, value_types)?;
                    LoweredScalarBranchTerminator::Return { expression }
                }
            }
            CheckedScalarStateTerminator::Crash { statement_ordinal } => {
                LoweredScalarBranchTerminator::Crash(lower_checked_crash_exit(
                    checked,
                    machine,
                    state.state,
                    *statement_ordinal,
                    &identity_reshuffles.source_claims,
                )?)
            }
            CheckedScalarStateTerminator::Conditional {
                guard_statement_ordinal,
                when_true,
                when_false,
            } => {
                branch_destinations::validate_coordinates(
                    *guard_statement_ordinal,
                    when_true,
                    when_false,
                )?;
                let LoweredDirectExpression::Boolean {
                    expression: condition,
                } = scalar_bindings.expression_at(
                    checked,
                    state.state,
                    *guard_statement_ordinal,
                    CheckedScalarExpressionRole::Guard,
                )?
                else {
                    return unsupported("checked scalar graph guard must be Boolean");
                };
                let condition = *condition;
                validate_short_circuit_expression(&condition)?;
                validate_boolean_parameter_types(&condition, value_types)?;

                let (when_true_target, when_true_arguments) =
                    branch_destinations::lower_destination(
                        checked,
                        states,
                        state.state,
                        value_types,
                        when_true,
                        scalar_bindings,
                        result_type,
                        return_sink,
                        &mut computations,
                    )?;
                let (when_false_target, when_false_arguments) =
                    branch_destinations::lower_destination(
                        checked,
                        states,
                        state.state,
                        value_types,
                        when_false,
                        scalar_bindings,
                        result_type,
                        return_sink,
                        &mut computations,
                    )?;
                LoweredScalarBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                }
            }
            CheckedScalarStateTerminator::Jump(successor) => {
                if successor.is_continuation {
                    return unsupported(
                        "an unconditional scalar jump cannot select continuation arguments",
                    );
                }
                let (target, arguments) = lower_scalar_graph_successor(
                    checked,
                    states,
                    state.state,
                    value_types,
                    successor,
                    scalar_bindings,
                    &mut computations,
                )?;
                LoweredScalarBranchTerminator::Jump { target, arguments }
            }
        };
        lowered_states.push(prepared.finish(state.state, terminator, &mut computations)?);
    }

    if return_sink.is_some() {
        // Return expressions are arguments to this private identity block.
        // Existing conditional argument lowering evaluates them only in the
        // selected arm; no source state or executable value is manufactured.
        lowered_states.push(LoweredScalarBranchState {
            parameter_types: vec![result_type],
            bindings: Vec::new(),
            terminator: LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Parameter {
                    position: 0,
                    scalar_type: result_type,
                },
            },
        });
    }

    lowered_states.extend(computations.finish());
    let successors = lowered_states
        .iter()
        .map(|state| match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, .. } => vec![*target],
            LoweredScalarBranchTerminator::Conditional {
                when_true_target,
                when_false_target,
                ..
            } => vec![*when_true_target, *when_false_target],
            _ => Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut indegree = vec![0usize; lowered_states.len()];
    for target in successors.iter().flatten() {
        let Some(degree) = indegree.get_mut(*target) else {
            return unsupported("scalar computation successor is outside its graph");
        };
        *degree += 1;
    }
    if indegree[0] != 0 || indegree[1..].contains(&0) {
        return unsupported(
            "scalar graph control must be rooted at the machine entry and reach every state",
        );
    }
    let mut visited = vec![false; lowered_states.len()];
    let mut active = vec![false; lowered_states.len()];
    validate_scalar_graph(0, &successors, &mut visited, &mut active)?;
    if visited.iter().any(|visited| !*visited) {
        return unsupported("scalar graph control contains an unreachable state");
    }

    let has_crash = lowered_states.iter().any(|state| {
        matches!(&state.terminator, LoweredScalarBranchTerminator::Crash(_))
            || state.bindings.iter().any(|binding| {
                matches!(binding, LoweredScalarBinding::DirectCall(call)
                        if !call.crash_continuations.is_empty())
            })
    });
    let has_return = lowered_states.iter().any(|state| {
        matches!(
            &state.terminator,
            LoweredScalarBranchTerminator::Return { .. }
        )
    });
    let expected_value = evaluate_known_scalar_graph(&lowered_states);
    let plan = closed_scalar_contract_plan(checked, machine)?;
    let has_predicates = plan
        .requires()
        .iter()
        .chain(plan.ensures())
        .any(|clause| matches!(clause, Some(ClosedScalarContractValue::Predicate(_))));
    let contract = if contract_mode == ScalarContractMode::EmbeddedByEnclosingCall {
        PreparedScalarContract::Empty
    } else if has_return {
        if contract_mode == ScalarContractMode::StandaloneProofOnlyFloatResult
            && exact_direct_result_float_meaning_reflexivity_contract(
                checked,
                machine,
                result_type,
                has_crash,
            )
        {
            PreparedScalarContract::Empty
        } else if has_predicates {
            if plan.has_outcome_specific_clauses()
                || plan
                    .requires()
                    .iter()
                    .chain(plan.ensures())
                    .any(Option::is_none)
            {
                return unsupported("scalar contract contains an unsupported clause");
            }
            PreparedScalarContract::Predicates(plan.clone())
        } else {
            PreparedScalarContract::ClosedLiteral(validate_closed_scalar_contract(
                checked,
                machine,
                result_type,
                expected_value,
                // A published crash clause is a ceiling, not a requirement
                // that the body retain a reachable crash. Checked selection
                // can eliminate every crashing RHS while preserving that API.
                has_crash || closed_scalar_contract_plan(checked, machine)?.has_crash_clauses(),
            )?)
        }
    } else {
        let contract = closed_scalar_contract_plan(checked, machine)?;
        if contract.has_outcome_specific_clauses()
            || !contract.requires().is_empty()
            || !contract.ensures().is_empty()
        {
            return unsupported("an all-crash scalar graph cannot declare a value contract");
        }
        PreparedScalarContract::Empty
    };
    Ok(PreparedScalarMachine {
        source_machine: machine,
        states: lowered_states,
        result_type,
        contract,
        crash_routes: lower_checked_crash_routes(checked, machine)?,
        identity_reshuffles,
        partition_compositions,
    })
}

#[allow(clippy::too_many_arguments)]
fn lower_checked_direct_call_binding(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    binding_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    argument_count: u32,
    result_type: ScalarType,
    caller_value_types: &[ScalarType],
    scalar_bindings: &storage::ScalarBindings,
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let arguments = (0..argument_count)
        .map(|argument_ordinal| {
            scalar_bindings.expression_at(
                checked,
                caller_state,
                statement_ordinal,
                CheckedScalarExpressionRole::CallArgument {
                    binding_ordinal,
                    argument_ordinal,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    lower_scalar_call(
        checked,
        caller_machine,
        caller_state,
        statement_ordinal,
        target_machine,
        target_state,
        call_ordinal,
        result_type,
        caller_value_types,
        arguments,
        ScalarCallCrashScope::CallerValues,
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_scalar_call(
    checked: &CheckedTrees,
    caller_machine: symbols::SymbolHandle,
    caller_state: symbols::SymbolHandle,
    statement_ordinal: u32,
    target_machine: symbols::SymbolHandle,
    target_state: symbols::SymbolHandle,
    call_ordinal: u32,
    result_type: ScalarType,
    caller_value_types: &[ScalarType],
    arguments: Vec<LoweredDirectExpression>,
    crash_scope: ScalarCallCrashScope,
) -> Result<LoweredDirectCallBinding, LoweringError> {
    let target_graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no source-independent checked graph",
        ))?;
    let target_entry = target_graph
        .states
        .first()
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked entry state",
        ))?;
    if target_entry.state != target_state {
        return unsupported("direct scalar call must target the callee entry state");
    }
    if terminal_scalar_type(target_entry.result_type)? != result_type {
        return unsupported("direct scalar call result type must match its local binding");
    }
    if arguments.len() != target_entry.parameter_types.len() {
        return unsupported("direct scalar call argument count must match the callee signature");
    }
    for (expression, target_type) in arguments.iter().zip(&target_entry.parameter_types) {
        if expression.scalar_type() != terminal_scalar_type(*target_type)? {
            return unsupported(
                "checked scalar call argument type must match its callee parameter",
            );
        }
        validate_direct_parameter_types(expression, caller_value_types)?;
    }
    let checked_call = checked
        .facts
        .contract_plans
        .for_machine(caller_machine)
        .and_then(|plan| {
            plan.crash
                .checked_call_at(caller_state, statement_ordinal, call_ordinal)
        })
        .ok_or(LoweringError::Unsupported(
            "direct scalar call has no matching checked crash-refinement row",
        ))?;
    if checked_call.target_machine() != target_machine
        || checked_call.target_state() != target_state
    {
        return unsupported("checked scalar call target disagrees with crash refinement");
    }
    let target_contract = checked
        .facts
        .contract_plans
        .for_machine(target_machine)
        .ok_or(LoweringError::Unsupported(
            "direct scalar call target has no checked contract plan",
        ))?;
    if checked_call.target_contract_report_fingerprint() != target_contract.report_fingerprint
        || checked_call.target_contract_commitment() != target_contract.commitment
    {
        return unsupported("checked scalar call target contract identity disagrees");
    }
    // Computed arguments have already become values. Bind the pinned callee
    // routes to those values, as ordinary staged calls do, rather than trying
    // to turn their effectful source expressions into pure caller predicates.
    let crash_continuations = match crash_scope {
        ScalarCallCrashScope::CallerValues => checked_call.surviving_buckets(),
        ScalarCallCrashScope::Arguments => target_contract.crash.published(),
    };
    if crash_continuations.iter().any(|bucket| {
        bucket.alternative_guards().iter().any(|guard| {
            matches!(guard, checked_trees::CrashRouteGuard::Predicate(predicate)
                if predicate.scalar_expression().is_none())
        })
    }) {
        return unsupported("direct scalar call crash continuation lacks a checked scalar term");
    }
    Ok(LoweredDirectCallBinding {
        source_coordinate: SourceCallCoordinate {
            state: caller_state,
            statement_index: usize::try_from(statement_ordinal).map_err(|_| {
                LoweringError::Unsupported("scalar call statement ordinal exceeds usize")
            })?,
            call_ordinal: usize::try_from(call_ordinal)
                .map_err(|_| LoweringError::Unsupported("scalar call ordinal exceeds usize"))?,
        },
        target_machine,
        result_type,
        arguments,
        crash_continuations: crash_continuations.to_vec(),
        crash_scope,
        parameter_relative_crash_routes: target_contract.crash.published().to_vec(),
    })
}

fn lower_scalar_graph_successor(
    checked: &CheckedTrees,
    states: &[checked_trees::CheckedScalarStateGraph],
    source_state: symbols::SymbolHandle,
    source_value_types: &[ScalarType],
    successor: &CheckedScalarSuccessor,
    scalar_bindings: &storage::ScalarBindings,
    computations: &mut computations::Expansion<'_>,
) -> Result<(usize, Vec<LoweredDirectExpression>), LoweringError> {
    let target = states
        .iter()
        .position(|candidate| candidate.state == successor.target)
        .ok_or(LoweringError::Unsupported(
            "scalar graph successor must belong to the selected machine",
        ))?;
    let target_parameter_types = &states[target].parameter_types;
    if usize::try_from(successor.argument_count).ok() != Some(target_parameter_types.len()) {
        return unsupported(
            "scalar graph successor bindings must match the target parameter count",
        );
    }
    if let Some(entry) = computations.successor(
        source_state,
        successor,
        scalar_bindings,
        source_value_types,
        target,
        target_parameter_types,
    )? {
        return Ok((entry, computations::parameters(source_value_types)));
    }
    let arguments = (0..successor.argument_count)
        .zip(target_parameter_types)
        .map(|(argument_ordinal, target_type)| {
            let target_type = terminal_scalar_type(*target_type)?;
            let expression = scalar_bindings.expression_at(
                checked,
                source_state,
                successor.statement_ordinal,
                if successor.is_continuation {
                    CheckedScalarExpressionRole::TransitionContinuationArgument { argument_ordinal }
                } else {
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                },
            )?;
            validate_direct_parameter_types(&expression, source_value_types)?;
            (expression.scalar_type() == target_type)
                .then_some(expression)
                .ok_or(LoweringError::Unsupported(
                    "checked scalar successor expression type must match its target",
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((target, arguments))
}

pub(super) fn lower_checked_scalar_expression_at(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement_ordinal: u32,
    role: CheckedScalarExpressionRole,
) -> Result<LoweredDirectExpression, LoweringError> {
    let expression = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(state, statement_ordinal, role)
        .ok_or(LoweringError::Unsupported(
            "scalar expression has no source-independent checked value plan",
        ))?;
    lower_checked_scalar_expression(expression)
}

pub(super) fn lower_checked_scalar_expression(
    expression: &CheckedScalarExpression,
) -> Result<LoweredDirectExpression, LoweringError> {
    match expression {
        CheckedScalarExpression::StorageRead { .. } => {
            unsupported("scalar storage read requires an exact current storage mapping")
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Parameter {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => Ok(LoweredDirectExpression::Local {
            position: *position,
            scalar_type: terminal_scalar_type(*primitive_type)?,
        }),
        CheckedScalarExpression::StructuralParameterField { .. } => unsupported(
            "structural parameter fields are retained only inside structural crash predicates",
        ),
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let scalar_type = integer_landing_scalar_type(literal)?;
            Ok(LoweredDirectExpression::IntegerLiteral {
                value: integer_value(literal, scalar_type)?,
                scalar_type,
            })
        }
        CheckedScalarExpression::IeeeFloatLiteral { value } => {
            Ok(LoweredDirectExpression::IeeeFloatLiteral { value: *value })
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => Ok(LoweredDirectExpression::IntegerBinary {
            kind: match kind {
                CheckedIntegerBinaryKind::ExactAdd => LoweredIntegerBinaryKind::ExactAdd,
                CheckedIntegerBinaryKind::ExactSubtract => LoweredIntegerBinaryKind::ExactSubtract,
                CheckedIntegerBinaryKind::ExactMultiply => LoweredIntegerBinaryKind::ExactMultiply,
                CheckedIntegerBinaryKind::ExactDivide => LoweredIntegerBinaryKind::ExactDivide,
                CheckedIntegerBinaryKind::ExactRemainder => {
                    LoweredIntegerBinaryKind::ExactRemainder
                }
                CheckedIntegerBinaryKind::WrappingDivide => {
                    LoweredIntegerBinaryKind::WrappingDivide
                }
                CheckedIntegerBinaryKind::WrappingRemainder => {
                    LoweredIntegerBinaryKind::WrappingRemainder
                }
                CheckedIntegerBinaryKind::SaturatingDivide => {
                    LoweredIntegerBinaryKind::SaturatingDivide
                }
                CheckedIntegerBinaryKind::SaturatingRemainder => {
                    LoweredIntegerBinaryKind::SaturatingRemainder
                }
                CheckedIntegerBinaryKind::WrappingAdd => LoweredIntegerBinaryKind::WrappingAdd,
                CheckedIntegerBinaryKind::SaturatingAdd => LoweredIntegerBinaryKind::SaturatingAdd,
                CheckedIntegerBinaryKind::WrappingSubtract => {
                    LoweredIntegerBinaryKind::WrappingSubtract
                }
                CheckedIntegerBinaryKind::SaturatingSubtract => {
                    LoweredIntegerBinaryKind::SaturatingSubtract
                }
                CheckedIntegerBinaryKind::WrappingMultiply => {
                    LoweredIntegerBinaryKind::WrappingMultiply
                }
                CheckedIntegerBinaryKind::SaturatingMultiply => {
                    LoweredIntegerBinaryKind::SaturatingMultiply
                }
                CheckedIntegerBinaryKind::BitwiseAnd => LoweredIntegerBinaryKind::BitwiseAnd,
                CheckedIntegerBinaryKind::BitwiseOr => LoweredIntegerBinaryKind::BitwiseOr,
                CheckedIntegerBinaryKind::BitwiseXor => LoweredIntegerBinaryKind::BitwiseXor,
                CheckedIntegerBinaryKind::WrappingShiftLeft => {
                    LoweredIntegerBinaryKind::WrappingShiftLeft
                }
                CheckedIntegerBinaryKind::WrappingShiftRight => {
                    LoweredIntegerBinaryKind::WrappingShiftRight
                }
                CheckedIntegerBinaryKind::ExactShiftLeft => {
                    LoweredIntegerBinaryKind::ExactShiftLeft
                }
                CheckedIntegerBinaryKind::ExactShiftRight => {
                    LoweredIntegerBinaryKind::ExactShiftRight
                }
            },
            scalar_type: terminal_scalar_type(*primitive_type)?,
            left: Box::new(lower_checked_scalar_expression(left)?),
            right: Box::new(lower_checked_scalar_expression(right)?),
        }),
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => Ok(LoweredDirectExpression::IntegerWiden {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            ..
        } => Ok(LoweredDirectExpression::IntegerExactCast {
            scalar_type: terminal_scalar_type(*primitive_type)?,
            operand: Box::new(lower_checked_scalar_expression(operand)?),
        }),
        CheckedScalarExpression::Boolean(expression) => Ok(LoweredDirectExpression::Boolean {
            expression: Box::new(lower_checked_boolean_expression(expression)?),
        }),
    }
}

fn lower_checked_boolean_expression(
    expression: &CheckedBooleanExpression,
) -> Result<LoweredBooleanReturnExpression, LoweringError> {
    Ok(match expression {
        CheckedBooleanExpression::Constant(value) => {
            LoweredBooleanReturnExpression::Constant { value: *value }
        }
        CheckedBooleanExpression::Parameter { position } => {
            LoweredBooleanReturnExpression::Parameter {
                position: *position,
            }
        }
        CheckedBooleanExpression::StorageRead { .. } => {
            return unsupported("Boolean storage read requires an exact current storage mapping");
        }
        CheckedBooleanExpression::Local { position } => LoweredBooleanReturnExpression::Local {
            position: *position,
        },
        CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => {
            let path = path
                .iter()
                .map(|segment| match segment {
                    checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                        Ok(identity.clone())
                    }
                    checked_trees::CheckedStructuralPredicatePathSegment::Case(_) => {
                        unsupported("case-payload predicates are contract-only")
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            LoweredBooleanReturnExpression::UnresolvedStructuralParameterField {
                parameter_position: *parameter_position,
                path,
            }
        }
        CheckedBooleanExpression::Not(operand) => LoweredBooleanReturnExpression::Not {
            operand: Box::new(lower_checked_boolean_expression(operand)?),
        },
        CheckedBooleanExpression::Equal { left, right } => LoweredBooleanReturnExpression::Equal {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            LoweredBooleanReturnExpression::IntegerComparison {
                kind: match kind {
                    CheckedIntegerComparisonKind::Equal => LoweredIntegerComparisonKind::Equal,
                    CheckedIntegerComparisonKind::LessThan => {
                        LoweredIntegerComparisonKind::LessThan
                    }
                    CheckedIntegerComparisonKind::LessOrEqual => {
                        LoweredIntegerComparisonKind::LessOrEqual
                    }
                },
                left: Box::new(lower_checked_scalar_expression(left)?),
                right: Box::new(lower_checked_scalar_expression(right)?),
            }
        }
        CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => {
            return unsupported("structural equality is contract-only terminal vocabulary");
        }
        CheckedBooleanExpression::And { left, right } => LoweredBooleanReturnExpression::And {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
        CheckedBooleanExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(lower_checked_boolean_expression(left)?),
            right: Box::new(lower_checked_boolean_expression(right)?),
        },
    })
}

pub(super) fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::StructuralField { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean type validation")
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("scalar graph guard parameters must be Boolean")
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_boolean_parameter_types(operand, parameter_types)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_boolean_parameter_types(left, parameter_types)?;
            validate_boolean_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
    }
}

pub(super) fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("scalar graph integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. }
        | LoweredDirectExpression::IeeeFloatLiteral { .. } => Ok(()),
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

fn validate_scalar_graph(
    state: usize,
    successors: &[Vec<usize>],
    visited: &mut [bool],
    active: &mut [bool],
) -> Result<(), LoweringError> {
    if active[state] {
        return unsupported("scalar graph control must be acyclic");
    }
    if visited[state] {
        return Ok(());
    }
    active[state] = true;
    for successor in &successors[state] {
        validate_scalar_graph(*successor, successors, visited, active)?;
    }
    active[state] = false;
    visited[state] = true;
    Ok(())
}

pub(super) fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
        LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
        LoweredBooleanReturnExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            true
        }
    }
}

pub(super) fn direct_expression_contains_short_circuit(
    expression: &LoweredDirectExpression,
) -> bool {
    matches!(
        expression,
        LoweredDirectExpression::Boolean { expression }
            if contains_short_circuit(expression)
    )
}

fn scalar_binding_contains_short_circuit(binding: &LoweredScalarBinding) -> bool {
    match binding {
        LoweredScalarBinding::Expression(expression) => {
            direct_expression_contains_short_circuit(expression)
        }
        LoweredScalarBinding::DirectCall(call) => call
            .arguments
            .iter()
            .any(direct_expression_contains_short_circuit),
    }
}

pub(super) fn staged_short_circuit_bindings_terminator(
    bindings: &[LoweredScalarBinding],
    terminator: &LoweredScalarBranchTerminator,
) -> Option<(Vec<LoweredScalarBinding>, LoweredScalarBranchTerminator)> {
    if !bindings.iter().any(scalar_binding_contains_short_circuit) {
        return None;
    }
    Some((bindings.to_vec(), terminator.clone()))
}

fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean validation")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KnownDirectScalar {
    Boolean(bool),
    Integer(IntegerValue),
}

fn evaluate_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<KnownDirectScalar> {
    match expression {
        LoweredDirectExpression::Parameter { position, .. }
        | LoweredDirectExpression::Local { position, .. } => {
            parameters.get(*position).copied().flatten()
        }
        LoweredDirectExpression::IntegerLiteral { value, .. } => {
            Some(KnownDirectScalar::Integer(*value))
        }
        LoweredDirectExpression::IeeeFloatLiteral { .. } => None,
        LoweredDirectExpression::IntegerBinary {
            kind,
            scalar_type,
            left,
            right,
        } => {
            let count_type = right.scalar_type();
            let KnownDirectScalar::Integer(left) = evaluate_direct_expression(left, parameters)?
            else {
                return None;
            };
            let KnownDirectScalar::Integer(right) = evaluate_direct_expression(right, parameters)?
            else {
                return None;
            };
            evaluate_lowered_integer_binary(*kind, *scalar_type, count_type, left, right)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(operand) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            integer_type
                .bitwise_not(operand)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerWiden {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .widen_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::IntegerExactCast {
            scalar_type,
            operand,
        } => {
            let ScalarType::Integer(source_type) = operand.scalar_type() else {
                return None;
            };
            let ScalarType::Integer(target_type) = scalar_type else {
                return None;
            };
            let KnownDirectScalar::Integer(value) =
                evaluate_direct_expression(operand, parameters)?
            else {
                return None;
            };
            source_type
                .exact_cast_value_to(*target_type, value)
                .map(KnownDirectScalar::Integer)
        }
        LoweredDirectExpression::Boolean { expression } => {
            evaluate_compile_known_boolean_expression(expression, parameters)
                .map(KnownDirectScalar::Boolean)
        }
    }
}

fn evaluate_integer_direct_expression(
    expression: &LoweredDirectExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<IntegerValue> {
    let KnownDirectScalar::Integer(value) = evaluate_direct_expression(expression, parameters)?
    else {
        return None;
    };
    Some(value)
}

fn evaluate_lowered_integer_binary(
    kind: LoweredIntegerBinaryKind,
    scalar_type: ScalarType,
    count_type: ScalarType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return None;
    };
    match kind {
        LoweredIntegerBinaryKind::BitwiseAnd => integer_type.bitwise_and(left, right),
        LoweredIntegerBinaryKind::BitwiseOr => integer_type.bitwise_or(left, right),
        LoweredIntegerBinaryKind::BitwiseXor => integer_type.bitwise_xor(left, right),
        LoweredIntegerBinaryKind::WrappingShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::WrappingShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.wrapping_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftLeft => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_left(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactShiftRight => {
            let ScalarType::Integer(count_type) = count_type else {
                return None;
            };
            integer_type.exact_shift_right(left, count_type, right)
        }
        LoweredIntegerBinaryKind::ExactAdd => integer_type.exact_add(left, right),
        LoweredIntegerBinaryKind::ExactSubtract => integer_type.exact_sub(left, right),
        LoweredIntegerBinaryKind::ExactMultiply => integer_type.exact_mul(left, right),
        LoweredIntegerBinaryKind::ExactDivide => integer_type.exact_div(left, right),
        LoweredIntegerBinaryKind::ExactRemainder => integer_type.exact_rem(left, right),
        LoweredIntegerBinaryKind::WrappingDivide => integer_type.wrapping_div(left, right),
        LoweredIntegerBinaryKind::WrappingRemainder => integer_type.wrapping_rem(left, right),
        LoweredIntegerBinaryKind::SaturatingDivide => integer_type.saturating_div(left, right),
        LoweredIntegerBinaryKind::SaturatingRemainder => integer_type.saturating_rem(left, right),
        LoweredIntegerBinaryKind::WrappingAdd => integer_type.wrapping_add(left, right),
        LoweredIntegerBinaryKind::SaturatingAdd => integer_type.saturating_add(left, right),
        LoweredIntegerBinaryKind::WrappingSubtract => integer_type.wrapping_sub(left, right),
        LoweredIntegerBinaryKind::SaturatingSubtract => integer_type.saturating_sub(left, right),
        LoweredIntegerBinaryKind::WrappingMultiply => integer_type.wrapping_mul(left, right),
        LoweredIntegerBinaryKind::SaturatingMultiply => integer_type.saturating_mul(left, right),
    }
}

fn evaluate_compile_known_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    parameters: &[Option<KnownDirectScalar>],
) -> Option<bool> {
    match expression {
        LoweredBooleanReturnExpression::Constant { value } => Some(*value),
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            let KnownDirectScalar::Boolean(value) = parameters.get(*position).copied().flatten()?
            else {
                return None;
            };
            Some(value)
        }
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. } => None,
        LoweredBooleanReturnExpression::Not { operand } => Some(
            !evaluate_compile_known_boolean_expression(operand, parameters)?,
        ),
        LoweredBooleanReturnExpression::Equal { left, right } => Some(
            evaluate_compile_known_boolean_expression(left, parameters)?
                == evaluate_compile_known_boolean_expression(right, parameters)?,
        ),
        LoweredBooleanReturnExpression::IntegerComparison { kind, left, right } => {
            let ScalarType::Integer(integer_type) = left.scalar_type() else {
                return None;
            };
            let left = evaluate_integer_direct_expression(left, parameters)?;
            let right = evaluate_integer_direct_expression(right, parameters)?;
            match kind {
                LoweredIntegerComparisonKind::Equal => Some(left == right),
                LoweredIntegerComparisonKind::LessThan => {
                    Some(integer_type.compare(left, right)?.is_lt())
                }
                LoweredIntegerComparisonKind::LessOrEqual => {
                    Some(!integer_type.compare(left, right)?.is_gt())
                }
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                evaluate_compile_known_boolean_expression(right, parameters)
            } else {
                Some(false)
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => {
            let left = evaluate_compile_known_boolean_expression(left, parameters)?;
            if left {
                Some(true)
            } else {
                evaluate_compile_known_boolean_expression(right, parameters)
            }
        }
    }
}

fn lower_content_evidence(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
) -> Result<
    (
        LoweredContentIdentityReshuffles,
        LoweredContentPartitionCompositions,
    ),
    LoweringError,
> {
    let identity_facts = checked
        .facts
        .qualifications
        .content
        .identity_reshuffles
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let mut identity_reshuffles = lower_content_identity_reshuffles(&identity_facts)?;
    let partition_facts = checked
        .facts
        .qualifications
        .content
        .partition_compositions
        .iter()
        .filter(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
        .cloned()
        .collect::<Vec<_>>();
    let partition_compositions =
        lower_content_partition_compositions(&partition_facts, &mut identity_reshuffles)?;
    Ok((identity_reshuffles, partition_compositions))
}

fn closed_scalar_contract_plan(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
) -> Result<&ClosedScalarValueContractPlan, LoweringError> {
    checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
        .ok_or(LoweringError::Unsupported(
            "machine has no source-independent checked contract plan",
        ))
}

/// Recognize the one D40 contract shape whose entire value is proof-only.
/// Runtime scalar-contract lowering must not manufacture an IEEE comparison
/// or a `FloatMeaning` runtime value for this clause, so every checked source
/// coordinate is replayed before the clause is erased from `MachineContract`.
fn exact_direct_result_float_meaning_reflexivity_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    result_type: ScalarType,
    allow_crash_contracts: bool,
) -> bool {
    let Some(contract_plan) = checked
        .facts
        .contract_plans
        .for_machine(machine)
        .map(|plan| &plan.closed_scalar_values)
    else {
        return false;
    };
    if !contract_plan.requires().is_empty()
        || contract_plan.ensures() != [None]
        || contract_plan.has_outcome_specific_clauses()
        || (!allow_crash_contracts && contract_plan.has_crash_clauses())
    {
        return false;
    }
    let Some(source_machine) = checked
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
    else {
        return false;
    };
    let mut ensures_contracts =
        checked
            .machine_contracts(source_machine)
            .iter()
            .filter(|contract| {
                contract.binding.is_none()
                    && contract.kind == checked_trees::signature::SignatureContractKind::Ensures
            });
    let Some(ensures) = ensures_contracts.next() else {
        return false;
    };
    if ensures_contracts.next().is_some() {
        return false;
    }
    let [checked_trees::domain::ProofFact::Expression(source_expression)] =
        checked.proof_facts.span_or_empty(ensures.facts)
    else {
        return false;
    };
    let Some(projection) = checked
        .facts
        .proof
        .direct_result_float_meaning_reflexivity(machine, *source_expression)
    else {
        return false;
    };
    if projection.validate().is_err() {
        return false;
    }
    let checked_trees::CheckedFloatProjectionSource::DirectMachineResult(result) =
        projection.source
    else {
        return false;
    };
    if result.owner_machine != machine {
        return false;
    }
    matches!(
        (result_type, result.fallback.primitive, projection.operation),
        (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            PrimitiveType::F32,
            numerics::float_projection::FloatProjectionOperation::Meaning32,
        ) | (
            ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            PrimitiveType::F64,
            numerics::float_projection::FloatProjectionOperation::Meaning64,
        )
    )
}

fn validate_closed_scalar_contract(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    result_type: ScalarType,
    expected_value: Option<KnownDirectScalar>,
    allow_crash_contracts: bool,
) -> Result<KnownDirectScalar, LoweringError> {
    let contract = closed_scalar_contract_plan(checked, machine)?;
    let ([Some(requires)], [Some(ensures)]) = (contract.requires(), contract.ensures()) else {
        return unsupported("machine must have exactly one requires and one ensures clause");
    };
    if contract.has_outcome_specific_clauses()
        || (!allow_crash_contracts && contract.has_crash_clauses())
    {
        return unsupported("machine must have exactly one requires and one ensures clause");
    }
    let (requires, ensures) = match (result_type, requires, ensures) {
        (
            ScalarType::Boolean,
            ClosedScalarContractValue::Boolean(requires),
            ClosedScalarContractValue::Boolean(ensures),
        ) => (
            KnownDirectScalar::Boolean(*requires),
            KnownDirectScalar::Boolean(*ensures),
        ),
        (
            ScalarType::Integer(_),
            ClosedScalarContractValue::Integer(requires),
            ClosedScalarContractValue::Integer(ensures),
        ) => (
            KnownDirectScalar::Integer(integer_value(requires, result_type)?),
            KnownDirectScalar::Integer(integer_value(ensures, result_type)?),
        ),
        _ => return unsupported("contract scalar type must match the machine result type"),
    };
    if requires != ensures {
        return unsupported("requires and ensures must carry the same closed equality");
    }
    if expected_value.is_some_and(|expected| expected != requires) {
        return match result_type {
            ScalarType::Boolean => {
                unsupported("Boolean contract literal must match the compile-known result")
            }
            ScalarType::Integer(_) => {
                unsupported("contract literals must equal the executed literal")
            }
            ScalarType::IeeeFloat(_) => {
                unsupported("closed scalar contract evaluation does not carry IEEE float literals")
            }
        };
    }
    Ok(requires)
}

pub(super) fn integer_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    if primitive == PrimitiveType::Addr {
        return IntegerType::address(64)
            .map(ScalarType::Integer)
            .map_err(|_| LoweringError::InvalidPsiIntegerType);
    }
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr => unreachable!("address carrier handled above"),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return unsupported("only primitive integers are supported");
        }
    };
    IntegerType::new(sign, bits)
        .map(ScalarType::Integer)
        .map_err(|_| LoweringError::InvalidPsiIntegerType)
}

pub(super) fn terminal_scalar_type(primitive: PrimitiveType) -> Result<ScalarType, LoweringError> {
    match primitive {
        PrimitiveType::Bool => Ok(ScalarType::Boolean),
        PrimitiveType::F32 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary32)),
        PrimitiveType::F64 => Ok(ScalarType::IeeeFloat(IeeeFloatFormat::Binary64)),
        primitive => integer_scalar_type(primitive),
    }
}

pub(super) fn integer_landing_scalar_type(
    literal: &numerics::literals::IntegerLiteral,
) -> Result<ScalarType, LoweringError> {
    use numerics::literals::LandedIntegerType;

    let primitive = match literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?
        .landed_type
    {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => PrimitiveType::Addr,
    };
    integer_scalar_type(primitive)
}

pub(super) fn integer_value(
    literal: &numerics::literals::IntegerLiteral,
    scalar_type: ScalarType,
) -> Result<IntegerValue, LoweringError> {
    let ScalarType::Integer(integer_type) = scalar_type else {
        return Err(LoweringError::InvalidPsiIntegerType);
    };
    let landing = literal
        .landing()
        .ok_or(LoweringError::UnlandedIntegerLiteral)?;
    if landing.landed_type.bit_width() != u32::from(integer_type.bits())
        || landing.landed_type.is_signed() != (integer_type.sign() == IntegerSign::Signed)
    {
        return Err(LoweringError::IntegerLandingMismatch);
    }
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(
            literal
                .value_i64()
                .map(i128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            literal
                .value_u64()
                .map(u128::from)
                .ok_or(LoweringError::IntegerLiteralOutsideSupportedMagnitude)?,
        ),
    };
    if !integer_type.admits(value) {
        return Err(LoweringError::IntegerLiteralOutsidePsiType);
    }
    Ok(value)
}
