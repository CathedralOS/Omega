//! Scalar-graph terminal module assembly.

use super::*;

mod result_contract;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_scalar_graph_module(
    states: &[LoweredScalarBranchState],
    result_type: ScalarType,
    contract_value: Option<KnownDirectScalar>,
    result_predicate: Option<CheckedBooleanExpression>,
    crash_routes: Vec<checked_trees::CrashRouteBucket>,
    identity_reshuffles: LoweredContentIdentityReshuffles,
    partition_compositions: LoweredContentPartitionCompositions,
    terminal_machine: MachineId,
    identity_base: u64,
    machine_ids: &[(symbols::SymbolHandle, MachineId)],
    requirement_counts: &[(symbols::SymbolHandle, usize)],
) -> Result<LoweredTerminalPsi, LoweringError> {
    let parameters = states[0]
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, scalar_type)| ValueDeclaration {
            id: value_id(
                identity_base
                    .checked_add(
                        u64::try_from(index).expect("parameter index fits a semantic identity"),
                    )
                    .expect("parameter identity base admits the parameter index")
                    .checked_add(1)
                    .expect("parameter identity is nonzero"),
            ),
            scalar_type: *scalar_type,
        })
        .collect::<Vec<_>>();
    let crash_routes = lower_checked_crash_route_buckets(&crash_routes, &parameters)?;
    let mut next_value_identity = identity_base
        .checked_add(
            u64::try_from(parameters.len()).expect("parameter count fits a semantic identity"),
        )
        .expect("parameter count fits the machine identity namespace")
        .checked_add(1)
        .expect("generated identities follow parameter identities");
    let mut state_parameters = Vec::with_capacity(states.len());
    state_parameters.push(parameters.clone());
    for state in &states[1..] {
        state_parameters.push(
            state
                .parameter_types
                .iter()
                .map(|scalar_type| {
                    let parameter = ValueDeclaration {
                        id: value_id(next_value_identity),
                        scalar_type: *scalar_type,
                    };
                    next_value_identity = next_value_identity
                        .checked_add(1)
                        .expect("scalar graph block parameter identities advance");
                    parameter
                })
                .collect(),
        );
    }

    let mut all_operations = OperationBuffer::new(identity_base);
    let call_obligation_base = identity_base
        .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE / 2)
        .expect("call obligation range fits the machine identity namespace");
    let mut call_emission = CallEmissionContext {
        machine_ids,
        requirement_counts,
        next_obligation_identity: call_obligation_base,
        obligation_limit: identity_base
            .checked_add(TERMINAL_MACHINE_IDENTITY_STRIDE)
            .expect("machine identity namespace has a finite upper bound"),
    };
    let mut next_edge_identity = identity_base
        .checked_add(1)
        .expect("edge identity base admits one-based identities");
    let mut next_block_identity = identity_base
        .checked_add(u64::try_from(states.len()).expect("state count fits a semantic identity"))
        .expect("state count fits the machine identity namespace")
        .checked_add(1)
        .expect("conditional binding blocks follow source blocks");
    let mut pending_blocks = Vec::new();
    let mut inlined_blocks = Vec::new();
    let mut blocks = Vec::with_capacity(states.len());
    for (index, state) in states.iter().enumerate() {
        let operation_start = all_operations.len();
        let current_parameters = &state_parameters[index];
        let source_block = block_id(
            identity_base
                .checked_add(u64::try_from(index).expect("state index fits a semantic identity"))
                .expect("state index fits the machine identity namespace")
                .checked_add(1)
                .expect("block identity is nonzero"),
        );
        let source_block_parameters = if index == 0 {
            Vec::new()
        } else {
            current_parameters.clone()
        };
        let staged_short_circuit_terminator =
            staged_short_circuit_bindings_terminator(&state.bindings, &state.terminator);
        let mut current_values = current_parameters.clone();
        let mut current_value_types = state.parameter_types.clone();
        if let Some((binding_plans, continuation_plan)) = staged_short_circuit_terminator {
            let mut stage_block = source_block;
            let mut stage_parameters = current_parameters.clone();
            let mut stage_parameter_types = state.parameter_types.clone();
            let mut stage_block_parameters = source_block_parameters;
            for (binding_index, binding) in binding_plans.iter().enumerate() {
                let mut next_stage_types = stage_parameter_types.clone();
                next_stage_types.push(binding.scalar_type());
                let next_stage_parameters = next_stage_types
                    .iter()
                    .copied()
                    .map(|scalar_type| {
                        let parameter = ValueDeclaration {
                            id: value_id(next_value_identity),
                            scalar_type,
                        };
                        next_value_identity = next_value_identity
                            .checked_add(1)
                            .expect("staged local parameter identities advance");
                        parameter
                    })
                    .collect::<Vec<_>>();
                let next_stage =
                    if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean {
                        expression,
                    }) = binding
                        && contains_short_circuit(expression)
                    {
                        let decision = lower_boolean_value_decision(expression);
                        let decision_block_count = boolean_decision_block_count(&decision);
                        let first_child_identity = next_block_identity;
                        let next_stage =
                            block_id(
                                next_block_identity
                                    .checked_add(u64::try_from(decision_block_count - 1).expect(
                                        "staged Boolean child count fits a semantic identity",
                                    ))
                                    .expect("staged Boolean continuation identity advances"),
                            );
                        next_block_identity = next_stage
                            .get()
                            .checked_add(1)
                            .expect("staged Boolean block identities advance");
                        let carried_arguments = stage_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>();
                        let first_reserved_identity = if binding_index == 0 {
                            first_child_identity
                                .checked_sub(1)
                                .expect("staged Boolean blocks follow source blocks")
                        } else {
                            stage_block.get()
                        };
                        let mut decision_blocks = Vec::with_capacity(decision_block_count);
                        let entry = emit_reserved_boolean_tuple_stage_blocks(
                            &decision,
                            &stage_parameters,
                            stage_block_parameters,
                            next_stage,
                            &carried_arguments,
                            first_reserved_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut decision_blocks,
                        );
                        assert_eq!(entry.get(), first_reserved_identity);
                        let mut decision_blocks = decision_blocks
                            .into_iter()
                            .map(|block| block.expect("every staged Boolean block is finalized"));
                        let mut root = decision_blocks
                            .next()
                            .expect("staged short-circuit Boolean has a decision root");
                        if binding_index == 0 {
                            root.id = source_block;
                            blocks.push(root);
                        } else {
                            inlined_blocks.push(root);
                        }
                        inlined_blocks.extend(decision_blocks);
                        next_stage
                    } else if let LoweredScalarBinding::DirectCall(call) = binding
                        && call
                            .arguments
                            .iter()
                            .any(direct_expression_contains_short_circuit)
                    {
                        let (next_stage, mut call_blocks) = emit_staged_scalar_call_binding(
                            call,
                            &stage_parameters,
                            &stage_parameter_types,
                            stage_block_parameters,
                            stage_block,
                            &mut next_block_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut call_emission,
                        )?;
                        let root = call_blocks
                            .drain(..1)
                            .next()
                            .expect("a staged scalar call has an argument root");
                        if binding_index == 0 {
                            blocks.push(root);
                        } else {
                            inlined_blocks.push(root);
                        }
                        inlined_blocks.extend(call_blocks);
                        next_stage
                    } else {
                        let next_stage = block_id(next_block_identity);
                        next_block_identity = next_block_identity
                            .checked_add(1)
                            .expect("staged direct-local block identities advance");
                        let stage_operation_start = all_operations.len();
                        let value = emit_scalar_binding(
                            binding,
                            &stage_parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                            &mut call_emission,
                        )?;
                        let mut arguments = stage_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect::<Vec<_>>();
                        arguments.push(value);
                        let edge = edge_id(next_edge_identity);
                        next_edge_identity = next_edge_identity
                            .checked_add(1)
                            .expect("staged direct-local edge identity advances");
                        let block = Block {
                            id: stage_block,
                            parameters: stage_block_parameters,
                            operations: all_operations[stage_operation_start..].to_vec(),
                            terminator: Terminator::Jump {
                                edge,
                                target: next_stage,
                                arguments,
                                trivial_affine_discards: Vec::new(),
                            },
                        };
                        if binding_index == 0 {
                            blocks.push(block);
                        } else {
                            inlined_blocks.push(block);
                        }
                        next_stage
                    };
                stage_block = next_stage;
                stage_parameters = next_stage_parameters;
                stage_parameter_types = next_stage_types;
                stage_block_parameters = stage_parameters.clone();
            }

            if let LoweredScalarBranchTerminator::Return {
                expression: LoweredDirectExpression::Boolean { expression },
            } = &continuation_plan
                && contains_short_circuit(expression)
            {
                let decision = lower_boolean_value_decision(expression);
                let block_count = boolean_decision_block_count(&decision);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(block_count - 1)
                            .expect("staged Boolean return child count fits a semantic identity"),
                    )
                    .expect("staged Boolean return block identities advance");
                let (root, children) = emit_inlined_boolean_value_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    LoweredBooleanDecisionExit::Return,
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }
            if let LoweredScalarBranchTerminator::Jump { target, arguments } = &continuation_plan
                && let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
                && contains_short_circuit(expression)
            {
                let decision = lower_boolean_value_decision(expression);
                let block_count = boolean_decision_block_count(&decision);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(block_count - 1)
                            .expect("staged Boolean jump child count fits a semantic identity"),
                    )
                    .expect("staged Boolean jump block identities advance");
                let target = scalar_source_block(identity_base, *target);
                let (root, children) = emit_inlined_boolean_value_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    LoweredBooleanDecisionExit::Jump { target },
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }
            if let LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } = &continuation_plan
                && contains_short_circuit(condition)
            {
                let decision = lower_boolean_control_decision(
                    condition,
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: true,
                    }),
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: false,
                    }),
                );
                let decision_block_count = boolean_decision_test_count(&decision);
                debug_assert!(decision_block_count > 0);
                let first_synthetic_block = block_id(next_block_identity);
                next_block_identity = next_block_identity
                    .checked_add(
                        u64::try_from(decision_block_count - 1)
                            .expect("staged Boolean guard child count fits a semantic identity"),
                    )
                    .expect("staged Boolean guard block identities advance");
                let when_true = build_scalar_conditional_target(
                    *when_true_target,
                    when_true_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut next_block_identity,
                    &mut next_value_identity,
                    &mut pending_blocks,
                    identity_base,
                );
                let when_false = build_scalar_conditional_target(
                    *when_false_target,
                    when_false_arguments,
                    &stage_parameters,
                    &stage_parameter_types,
                    &mut next_block_identity,
                    &mut next_value_identity,
                    &mut pending_blocks,
                    identity_base,
                );
                let (root, children) = emit_inlined_boolean_guard_blocks(
                    &decision,
                    &stage_parameters,
                    stage_parameters.clone(),
                    &when_true,
                    &when_false,
                    stage_block,
                    first_synthetic_block,
                    &mut next_value_identity,
                    &mut next_edge_identity,
                    &mut all_operations,
                );
                inlined_blocks.push(root);
                inlined_blocks.extend(children);
                continue;
            }

            let operation_start = all_operations.len();
            let terminator = match continuation_plan {
                LoweredScalarBranchTerminator::Return { expression } => {
                    let value = emit_direct_expression(
                        &expression,
                        &stage_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean return edge identity advances");
                    Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value,
                    }
                }
                LoweredScalarBranchTerminator::Conditional {
                    condition,
                    when_true_target,
                    when_true_arguments,
                    when_false_target,
                    when_false_arguments,
                } => {
                    let condition = emit_boolean_expression(
                        &condition,
                        &stage_parameters,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let when_true = build_scalar_conditional_target(
                        when_true_target,
                        &when_true_arguments,
                        &stage_parameters,
                        &stage_parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        when_false_target,
                        &when_false_arguments,
                        &stage_parameters,
                        &stage_parameter_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_true_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean true edge identity advances");
                    let when_false_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("carried Boolean false edge identity advances");
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                    }
                }
                LoweredScalarBranchTerminator::Jump { target, arguments } => {
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("staged local jump edge identity advances");
                    if arguments
                        .iter()
                        .any(direct_expression_contains_short_circuit)
                    {
                        let target = build_scalar_conditional_target(
                            target,
                            &arguments,
                            &stage_parameters,
                            &stage_parameter_types,
                            &mut next_block_identity,
                            &mut next_value_identity,
                            &mut pending_blocks,
                            identity_base,
                        );
                        Terminator::Jump {
                            edge,
                            target: target.block,
                            arguments: target.arguments,
                            trivial_affine_discards: Vec::new(),
                        }
                    } else {
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                emit_direct_expression(
                                    argument,
                                    &stage_parameters,
                                    &mut next_value_identity,
                                    &mut all_operations,
                                )
                            })
                            .collect();
                        Terminator::Jump {
                            edge,
                            target: scalar_source_block(identity_base, target),
                            arguments,
                            trivial_affine_discards: Vec::new(),
                        }
                    }
                }
                LoweredScalarBranchTerminator::Crash(crash) => {
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("staged local crash edge identity advances");
                    Terminator::Crash {
                        edge,
                        cause: crash.cause,
                        site_guard: lower_checked_crash_predicates(&crash.site_guard, &parameters)?,
                        frontier_lower_bound: crash.frontier_lower_bound,
                    }
                }
            };
            inlined_blocks.push(Block {
                id: stage_block,
                parameters: stage_parameters,
                operations: all_operations[operation_start..].to_vec(),
                terminator,
            });
            continue;
        }
        for binding in &state.bindings {
            let id = emit_scalar_binding(
                binding,
                &current_values,
                &mut next_value_identity,
                &mut all_operations,
                &mut call_emission,
            )?;
            current_values.push(ValueDeclaration {
                id,
                scalar_type: binding.scalar_type(),
            });
            current_value_types.push(binding.scalar_type());
        }
        let terminator_operation_start = all_operations.len();
        let terminator = match &state.terminator {
            LoweredScalarBranchTerminator::Jump { target, arguments } => {
                if let [LoweredDirectExpression::Boolean { expression }] = arguments.as_slice()
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("Boolean binding child count fits a semantic identity"),
                        )
                        .expect("Boolean binding block identities advance");
                    let target = scalar_source_block(identity_base, *target);
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Jump { target },
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else if arguments
                    .iter()
                    .any(direct_expression_contains_short_circuit)
                {
                    let target = build_scalar_conditional_target(
                        *target,
                        arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("mixed tuple entry edge identity advances");
                    Terminator::Jump {
                        edge,
                        target: target.block,
                        arguments: target.arguments,
                        trivial_affine_discards: Vec::new(),
                    }
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| {
                            emit_direct_expression(
                                argument,
                                &current_values,
                                &mut next_value_identity,
                                &mut all_operations,
                            )
                        })
                        .collect();
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph jump edge identities advance");
                    Terminator::Jump {
                        edge,
                        target: scalar_source_block(identity_base, *target),
                        arguments,
                        trivial_affine_discards: Vec::new(),
                    }
                }
            }
            LoweredScalarBranchTerminator::Conditional {
                condition,
                when_true_target,
                when_true_arguments,
                when_false_target,
                when_false_arguments,
            } => {
                if contains_short_circuit(condition) {
                    let decision = lower_boolean_control_decision(
                        condition,
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: true,
                        }),
                        LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                            value: false,
                        }),
                    );
                    let decision_block_count = boolean_decision_test_count(&decision);
                    debug_assert!(decision_block_count > 0);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(decision_block_count - 1)
                                .expect("scalar graph guard child count fits a semantic identity"),
                        )
                        .expect("scalar graph guard block identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let (root, children) = emit_inlined_boolean_guard_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        &when_true,
                        &when_false,
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else {
                    let condition = emit_boolean_expression(
                        condition,
                        &current_values,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let when_true_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_false_edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph edge identities advance");
                    let when_true = build_scalar_conditional_target(
                        *when_true_target,
                        when_true_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    let when_false = build_scalar_conditional_target(
                        *when_false_target,
                        when_false_arguments,
                        &current_values,
                        &current_value_types,
                        &mut next_block_identity,
                        &mut next_value_identity,
                        &mut pending_blocks,
                        identity_base,
                    );
                    Terminator::Conditional {
                        condition,
                        when_true: SuccessorEdge {
                            edge: when_true_edge,
                            target: when_true.block,
                            arguments: when_true.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            edge: when_false_edge,
                            target: when_false.block,
                            arguments: when_false.arguments,
                            trivial_affine_discards: Vec::new(),
                        },
                    }
                }
            }
            LoweredScalarBranchTerminator::Return { expression } => {
                if let LoweredDirectExpression::Boolean { expression } = expression
                    && contains_short_circuit(expression)
                {
                    let decision = lower_boolean_value_decision(expression);
                    let block_count = boolean_decision_block_count(&decision);
                    let first_synthetic_block = block_id(next_block_identity);
                    next_block_identity = next_block_identity
                        .checked_add(
                            u64::try_from(block_count - 1)
                                .expect("scalar return child count fits a semantic identity"),
                        )
                        .expect("scalar return block identities advance");
                    let (root, children) = emit_inlined_boolean_value_blocks(
                        &decision,
                        &current_values,
                        source_block_parameters,
                        LoweredBooleanDecisionExit::Return,
                        source_block,
                        first_synthetic_block,
                        &mut next_value_identity,
                        &mut next_edge_identity,
                        &mut all_operations,
                    );
                    let mut root = root;
                    root.operations.splice(
                        0..0,
                        all_operations[operation_start..terminator_operation_start]
                            .iter()
                            .cloned(),
                    );
                    blocks.push(root);
                    inlined_blocks.extend(children);
                    continue;
                } else {
                    let value = emit_direct_expression(
                        expression,
                        &current_values,
                        &mut next_value_identity,
                        &mut all_operations,
                    );
                    let edge = edge_id(next_edge_identity);
                    next_edge_identity = next_edge_identity
                        .checked_add(1)
                        .expect("scalar graph return edge identities advance");
                    Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge,
                        value,
                    }
                }
            }
            LoweredScalarBranchTerminator::Crash(crash) => {
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("nested crash edge identities advance");
                Terminator::Crash {
                    edge,
                    cause: crash.cause,
                    site_guard: lower_checked_crash_predicates(&crash.site_guard, &parameters)?,
                    frontier_lower_bound: crash.frontier_lower_bound.clone(),
                }
            }
        };
        blocks.push(Block {
            id: source_block,
            parameters: source_block_parameters,
            operations: all_operations[operation_start..].to_vec(),
            terminator,
        });
    }
    blocks.extend(inlined_blocks);
    pending_blocks.sort_by_key(PendingNestedBlockGroup::first_id);
    for pending in pending_blocks {
        match pending {
            PendingNestedBlockGroup::ConditionalBinding(pending) => {
                let operation_start = all_operations.len();
                let arguments = pending
                    .arguments
                    .iter()
                    .map(|argument| {
                        emit_direct_expression(
                            argument,
                            &pending.parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                        )
                    })
                    .collect();
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("conditional binding jump edge identities advance");
                blocks.push(Block {
                    id: pending.id,
                    parameters: pending.parameters,
                    operations: all_operations[operation_start..].to_vec(),
                    terminator: Terminator::Jump {
                        edge,
                        target: pending.target,
                        arguments,
                        trivial_affine_discards: Vec::new(),
                    },
                });
            }
            PendingNestedBlockGroup::TupleBinding(pending) => {
                let mut pending_stage_blocks = Vec::new();
                let mut next_stage_identity = pending.first_id.get();
                for (index, argument) in pending.arguments.iter().enumerate() {
                    let parameters = &pending.stage_parameters[index];
                    let carried_arguments = parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    if let LoweredDirectExpression::Boolean { expression } = argument
                        && contains_short_circuit(expression)
                    {
                        let decision = lower_boolean_value_decision(expression);
                        let stage_block_count = boolean_decision_block_count(&decision);
                        let next_stage = block_id(
                            next_stage_identity
                                .checked_add(
                                    u64::try_from(stage_block_count)
                                        .expect("mixed tuple stage count fits a semantic identity"),
                                )
                                .expect("mixed tuple stage block identities advance"),
                        );
                        let mut stage_blocks = Vec::with_capacity(stage_block_count);
                        let entry = emit_reserved_boolean_tuple_stage_blocks(
                            &decision,
                            parameters,
                            parameters.clone(),
                            next_stage,
                            &carried_arguments,
                            next_stage_identity,
                            &mut next_value_identity,
                            &mut next_edge_identity,
                            &mut all_operations,
                            &mut stage_blocks,
                        );
                        assert_eq!(entry.get(), next_stage_identity);
                        pending_stage_blocks.extend(stage_blocks);
                        next_stage_identity = next_stage.get();
                    } else {
                        let operation_start = all_operations.len();
                        let value = emit_direct_expression(
                            argument,
                            parameters,
                            &mut next_value_identity,
                            &mut all_operations,
                        );
                        let mut arguments = carried_arguments;
                        arguments.push(value);
                        let next_stage = block_id(
                            next_stage_identity
                                .checked_add(1)
                                .expect("mixed tuple stage block identity advances"),
                        );
                        let edge = edge_id(next_edge_identity);
                        next_edge_identity = next_edge_identity
                            .checked_add(1)
                            .expect("mixed tuple stage edge identity advances");
                        pending_stage_blocks.push(Some(Block {
                            id: block_id(next_stage_identity),
                            parameters: parameters.clone(),
                            operations: all_operations[operation_start..].to_vec(),
                            terminator: Terminator::Jump {
                                edge,
                                target: next_stage,
                                arguments,
                                trivial_affine_discards: Vec::new(),
                            },
                        }));
                        next_stage_identity = next_stage.get();
                    }
                }
                let parameters = pending
                    .stage_parameters
                    .last()
                    .expect("mixed tuple has a convergence parameter set");
                let edge = edge_id(next_edge_identity);
                next_edge_identity = next_edge_identity
                    .checked_add(1)
                    .expect("mixed tuple convergence edge identity advances");
                pending_stage_blocks.push(Some(Block {
                    id: block_id(next_stage_identity),
                    parameters: parameters.clone(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        edge,
                        target: pending.target,
                        arguments: parameters[pending.original_parameter_count..]
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                        trivial_affine_discards: Vec::new(),
                    },
                }));
                blocks.extend(
                    pending_stage_blocks
                        .into_iter()
                        .map(|block| block.expect("every reserved mixed tuple block is finalized")),
                );
            }
        }
    }
    blocks.sort_by_key(|block| block.id);
    let result = ValueDeclaration {
        id: value_id(next_value_identity),
        scalar_type: result_type,
    };
    let mut resolved_partition_compositions = partition_compositions
        .compositions
        .into_iter()
        .map(|composition| {
            let Some(occurrence) = all_operations.source_calls.iter().find(|occurrence| {
                occurrence.source_state == composition.producer_coordinate.state
                    && occurrence.statement_index == composition.producer_coordinate.statement_index
                    && occurrence.call_ordinal == composition.producer_coordinate.call_ordinal
            }) else {
                return Err(LoweringError::ContentPartitionProducerOperationMissing);
            };
            if occurrence.source_target != composition.source_callable {
                return Err(LoweringError::ContentPartitionProducerTargetMismatch);
            }
            Ok(ContentPartitionComposition {
                producer_operation: occurrence.terminal_operation,
                source_report_fingerprint: composition.source_report_fingerprint,
                source_structural_places: composition.source_structural_places,
                source: composition.source,
                input_claims: composition.input_claims,
                substitutions: composition.substitutions,
                derived: composition.derived,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    resolved_partition_compositions.sort();
    let (requires, mut ensures, mut evidence) = match (result_type, contract_value) {
        (ScalarType::Boolean, Some(KnownDirectScalar::Boolean(value))) => {
            let literal = ScalarTerm::boolean(value);
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ReflexiveEquality),
                }],
            )
        }
        (ScalarType::Integer(integer_type), Some(KnownDirectScalar::Integer(value))) => {
            let literal = ScalarTerm::integer(integer_type, value)
                .expect("validated source contract fits the result type");
            let goal = Proposition::Equal(literal.clone(), literal);
            let obligation = obligation_id(
                identity_base
                    .checked_add(1)
                    .expect("contract obligation identity is one-based"),
            );
            (
                vec![goal.clone()],
                vec![ContractClause {
                    obligation,
                    proposition: goal,
                }],
                vec![ObligationEvidence {
                    obligation,
                    route: EvidenceRoute::KernelDerived(PrimitiveJudgment::ClosedIntegerRelation),
                }],
            )
        }
        (_, None) => (Vec::new(), Vec::new(), Vec::new()),
        _ => unreachable!("validated scalar contract matches the machine result type"),
    };
    if let Some(predicate) = result_predicate {
        let [clause] = ensures.as_mut_slice() else {
            return unsupported("result predicate has no corresponding scalar contract clause");
        };
        clause.proposition = result_contract::proposition(&predicate, result)?;
        // Reflexivity of the old literal tautology cannot prove this result
        // relation. Finalization derives its certificate from emitted exits.
        evidence.clear();
    }
    let mut structural_places = identity_reshuffles
        .structural_places
        .into_iter()
        .map(|place| (place.id, place.kind))
        .collect::<BTreeMap<_, _>>();
    for place in partition_compositions.structural_places {
        merge_content_place_declaration(&mut structural_places, place)
            .expect("checked lowering rejects conflicting structural places");
    }
    Ok(LoweredTerminalPsi {
        semantic_module: TerminalModule {
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: terminal_machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                id: terminal_machine,
                attachment: None,
                structural_parameters: Vec::new(),
                ranked_scc: None,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters,
                result: TerminalMachineResult::Scalar(result),
                structural_places: structural_places
                    .into_iter()
                    .map(|(id, kind)| StructuralPlaceDeclaration { id, kind })
                    .collect(),
                content_entry_claims: identity_reshuffles.entry_claims,
                content_identity_reshuffles: identity_reshuffles.reshuffles,
                content_partition_compositions: resolved_partition_compositions,
                entry: block_id(
                    identity_base
                        .checked_add(1)
                        .expect("machine entry block identity is one-based"),
                ),
                blocks,
                contract: MachineContract {
                    id: contract_id(terminal_machine.get()),
                    crash_routes,
                    requires,
                    ensures,
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        },
        proof_bundle: ProofBundle {
            recursive_components: Vec::new(),
            evidence_producers: Vec::new(),
            evidence,
        },
        debug_map: None,
        source_call_occurrences: all_operations.source_calls,
        selected_ieee_float_fma_occurrences: all_operations.selected_ieee_float_fmas,
    })
}
