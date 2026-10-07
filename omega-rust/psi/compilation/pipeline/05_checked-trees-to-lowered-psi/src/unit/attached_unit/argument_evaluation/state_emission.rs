//! Per-state scalar block emission: one branch state's decisions become a
//! block plus its dispatch-edge argument evaluations.
use super::super::super::{
    BlockId, SuccessorEdge, ValueId, block_id, boolean_decision_block_count,
    contains_short_circuit, emit_boolean_expression, emit_reserved_boolean_tuple_stage_blocks,
    emit_scalar_binding, lower_boolean_value_decision,
};
use super::super::{
    Block, LoweringError, Terminator, ValueDeclaration, allocate_dense,
    direct_expression_contains_short_circuit, edge_id, emit_direct_expression, unsupported,
    validate_direct_parameter_types, value_id,
};
use super::declarations;
use crate::emission::boolean_control::{
    LoweredBooleanDecision, LoweredBooleanDecisionTarget, boolean_guard_decision_block_count,
    emit_inlined_boolean_guard_blocks, lower_boolean_control_decision,
};
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_contracts::erased_proof_formal_declarations;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::{
    LoweredScalarBranchState, LoweredScalarBranchTerminator,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn emit_state(
    state: &LoweredScalarBranchState,
    mut block: BlockId,
    parameters: &[ValueDeclaration],
    caller_erased_formals: &[ValueDeclaration],
    source_erased_proof_formals: &[checked_trees::CheckedErasedProofParameterPlan],
    targets: &[BlockId],
    state_parameters: &[Vec<ValueDeclaration>],
    machine: Option<semantic_vocabulary::MachineId>,
    qualification_sets: &[terminal_psi::ScalarQualificationSet],
    coercions: &mut Vec<terminal_psi::ScalarQualificationCoercion>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut CallEmissionContext<'_>,
    blocks: &mut Vec<Block>,
) -> Result<(), LoweringError> {
    let erased_formals = declarations(&state.erased_formal_types, next_value)?;
    // Expansion-state order is not dominance order. Only observations made
    // within this state's path may be reused while emitting its decisions.
    operations.byte_lengths.clear();
    operations.element_lengths.clear();
    operations.field_byte_lengths.clear();
    let mut values = parameters.to_vec();
    let mut block_parameters = parameters.to_vec();
    let mut operation_start = operations.len();
    for binding in &state.bindings {
        if let LoweredScalarBinding::Expression(LoweredDirectExpression::Boolean { expression }) =
            binding
            && contains_short_circuit(expression)
        {
            let decision = lower_boolean_value_decision(expression);
            let count = boolean_decision_block_count(&decision);
            let reserved_root = *next_block;
            *next_block = next_block
                .checked_add(u64::try_from(count).map_err(|_| {
                    LoweringError::Unsupported("call Boolean block count exceeds u64")
                })?)
                .ok_or(LoweringError::Unsupported(
                    "call Boolean block identities exhausted",
                ))?;
            let continuation = block_id(allocate_dense(next_block)?);
            let mut types = values
                .iter()
                .map(|value| value.value_type())
                .collect::<Vec<_>>();
            types.push(binding.value_type(&types)?);
            let continuation_parameters = declarations(&types, next_value)?;
            let prefix = operations[operation_start..].to_vec();
            let mut decisions = Vec::new();
            emit_reserved_boolean_tuple_stage_blocks(
                &decision,
                &values,
                block_parameters,
                continuation,
                &values.iter().map(|value| value.id).collect::<Vec<_>>(),
                reserved_root,
                next_value,
                next_edge,
                operations,
                &mut decisions,
            );
            for (index, decision) in decisions.into_iter().enumerate() {
                let mut decision = decision.ok_or(LoweringError::Unsupported(
                    "call Boolean block is incomplete",
                ))?;
                if index == 0 {
                    decision.id = block;
                    decision.operations.splice(0..0, prefix.iter().cloned());
                }
                blocks.push(decision);
            }
            block = continuation;
            values = continuation_parameters.clone();
            block_parameters = continuation_parameters;
            operation_start = operations.len();
        } else {
            let value_type = binding.value_type(
                &values
                    .iter()
                    .map(|value| value.value_type())
                    .collect::<Vec<_>>(),
            )?;
            let id = emit_scalar_binding(
                binding,
                &values,
                caller_erased_formals,
                next_value,
                operations,
                calls,
            )?;
            values.push(ValueDeclaration {
                qualifications: value_type.qualifications,
                id,
                scalar_type: value_type.scalar_type,
            });
        }
    }
    crate::scalar_graph::scalar_graph_effects::emit(
        &state.structural_effects,
        &values,
        caller_erased_formals,
        next_value,
        operations,
        calls,
    )?;
    let mut arguments =
        |expressions: &[LoweredDirectExpression]| -> Result<Vec<ValueId>, LoweringError> {
            expressions
                .iter()
                .map(|expression| {
                    validate_direct_parameter_types(
                        expression,
                        &values
                            .iter()
                            .map(|value| value.scalar_type)
                            .collect::<Vec<_>>(),
                    )?;
                    if direct_expression_contains_short_circuit(expression) {
                        return unsupported(
                            "call computation transfer retains unexpanded Boolean control",
                        );
                    }
                    Ok(emit_direct_expression(
                        expression, &values, next_value, operations,
                    ))
                })
                .collect()
        };
    let terminator = match &state.terminator {
        LoweredScalarBranchTerminator::Qualify {
            target,
            arguments: qualify_arguments,
            erased_arguments: qualify_erased_arguments,
            erased_proof_arguments: qualify_erased_proof_arguments,
            structural_arguments: qualify_structural_arguments,
        } => {
            let mut terminator = crate::scalar_graph::scalar_graph_module::qualifications::emit(
                *targets.get(*target).ok_or(LoweringError::Unsupported(
                    "call computation qualification target is absent",
                ))?,
                edge_id(allocate_dense(next_edge)?),
                machine.ok_or(LoweringError::Unsupported(
                    "attached scalar qualification has no machine identity",
                ))?,
                qualify_arguments,
                qualify_structural_arguments,
                &values,
                state_parameters
                    .get(*target)
                    .ok_or(LoweringError::Unsupported(
                        "scalar qualification has no successor declarations",
                    ))?,
                qualification_sets,
                coercions,
            )?;
            let Terminator::Jump {
                erased_arguments: scalar_actuals,
                erased_proof_arguments: proof_actuals,
                ..
            } = &mut terminator
            else {
                return unsupported("scalar qualification lost its successor edge");
            };
            *scalar_actuals = qualify_erased_arguments
                .iter()
                .map(|argument| {
                    crate::proofs::crash_routes::lowered_direct_scalar_term(
                        argument,
                        &values,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            *proof_actuals = qualify_erased_proof_arguments
                .iter()
                .map(|argument| {
                    crate::scalar_graph::scalar_contracts::lowered_proof_term(
                        argument,
                        &values,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            terminator
        }
        LoweredScalarBranchTerminator::Jump {
            target,
            arguments: outgoing,
            erased_arguments,
            erased_proof_arguments,
            structural_arguments,
            cleanup_actions,
        } => Terminator::Jump {
            structural_arguments: structural_arguments.clone(),
            edge: edge_id(allocate_dense(next_edge)?),
            target: *targets.get(*target).ok_or(LoweringError::Unsupported(
                "call computation target is absent",
            ))?,
            arguments: arguments(outgoing)?,
            erased_arguments: erased_arguments
                .iter()
                .map(|argument| {
                    crate::proofs::crash_routes::lowered_direct_scalar_term(
                        argument,
                        &values,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            erased_proof_arguments: erased_proof_arguments
                .iter()
                .map(|term| {
                    crate::scalar_graph::scalar_contracts::lowered_proof_term(
                        term,
                        &values,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
            cleanup_actions: crate::unit::unit_cleanup::emit_prepared_cleanup_actions(
                cleanup_actions,
                calls,
            )?,
        },
        LoweredScalarBranchTerminator::Conditional {
            condition,
            when_true_target,
            when_true_arguments,
            when_true_erased_arguments,
            when_false_target,
            when_false_arguments,
            when_false_erased_arguments,
            when_true_erased_proof_arguments,
            when_false_erased_proof_arguments,
        } => {
            let when_true_arguments = arguments(when_true_arguments)?;
            let when_false_arguments = arguments(when_false_arguments)?;
            let erased_terms = |expressions: &[LoweredDirectExpression]| {
                expressions
                    .iter()
                    .map(|argument| {
                        crate::proofs::crash_routes::lowered_direct_scalar_term(
                            argument,
                            &values,
                            &erased_formals,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            let when_true_erased_arguments = erased_terms(when_true_erased_arguments)?;
            let when_false_erased_arguments = erased_terms(when_false_erased_arguments)?;
            let proof_terms =
                |terms: &[crate::scalar_graph::scalar_contracts::LoweredProofTerm]| {
                    terms
                        .iter()
                        .map(|term| {
                            crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                term,
                                &values,
                                &erased_formals,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()
                };
            let when_true_erased_proof_arguments = proof_terms(when_true_erased_proof_arguments)?;
            let when_false_erased_proof_arguments = proof_terms(when_false_erased_proof_arguments)?;
            if contains_short_circuit(condition) {
                if !when_true_erased_arguments.is_empty()
                    || !when_false_erased_arguments.is_empty()
                    || !when_true_erased_proof_arguments.is_empty()
                    || !when_false_erased_proof_arguments.is_empty()
                {
                    return unsupported(
                        "erased call operands do not stage through guard decisions",
                    );
                }
                let when_true_block = *targets
                    .get(*when_true_target)
                    .ok_or(LoweringError::Unsupported("call true target is absent"))?;
                let when_false_block = *targets
                    .get(*when_false_target)
                    .ok_or(LoweringError::Unsupported("call false target is absent"))?;
                let decision = lower_boolean_control_decision(
                    condition,
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: true,
                    }),
                    LoweredBooleanDecision::Value(LoweredBooleanReturnExpression::Constant {
                        value: false,
                    }),
                );
                let decision_tests = boolean_guard_decision_block_count(&decision);
                debug_assert!(decision_tests > 0);
                let first_synthetic_block = block_id(*next_block);
                *next_block = next_block
                    .checked_add(u64::try_from(decision_tests - 1).map_err(|_| {
                        LoweringError::Unsupported("call guard decision count exceeds u64")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "call guard decision identities exhausted",
                    ))?;
                let terminator_operation_start = operations.len();
                let (root, children) = emit_inlined_boolean_guard_blocks(
                    &decision,
                    &values,
                    block_parameters,
                    &LoweredBooleanDecisionTarget {
                        block: when_true_block,
                        arguments: when_true_arguments,
                    },
                    &LoweredBooleanDecisionTarget {
                        block: when_false_block,
                        arguments: when_false_arguments,
                    },
                    block,
                    first_synthetic_block,
                    next_value,
                    next_edge,
                    operations,
                );
                let mut root = root;
                root.erased_scalar_formals = erased_formals;
                root.erased_proof_formals =
                    erased_proof_formal_declarations(source_erased_proof_formals);
                root.operations.splice(
                    0..0,
                    operations[operation_start..terminator_operation_start]
                        .iter()
                        .cloned(),
                );
                blocks.push(root);
                blocks.extend(children);
                return Ok(());
            }
            let condition = emit_boolean_expression(condition, &values, next_value, operations);
            Terminator::Conditional {
                condition,
                when_true: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_true_target)
                        .ok_or(LoweringError::Unsupported("call true target is absent"))?,
                    arguments: when_true_arguments,
                    erased_arguments: when_true_erased_arguments,
                    erased_proof_arguments: when_true_erased_proof_arguments,
                    cleanup_actions: Vec::new(),
                },
                when_false: SuccessorEdge {
                    structural_arguments: Vec::new(),
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_false_target)
                        .ok_or(LoweringError::Unsupported("call false target is absent"))?,
                    arguments: when_false_arguments,
                    erased_arguments: when_false_erased_arguments,
                    erased_proof_arguments: when_false_erased_proof_arguments,
                    cleanup_actions: Vec::new(),
                },
            }
        }
        LoweredScalarBranchTerminator::CaseDispatch {
            source,
            selected,
            cases,
            payloads,
            when_true_target,
            when_true_arguments,
            when_true_erased_arguments,
            when_true_erased_proof_arguments,
            when_false_target,
            when_false_arguments,
            when_false_erased_arguments,
            when_false_erased_proof_arguments,
        } => {
            // Case edges forward no values, so the selected outcome continues
            // through a parameter-only block: its declared parameters are the
            // case's scalar payloads, matching the edge's `payload_fields`
            // roster. The dominating state's values stay in scope there.
            let mut dispatch_namespace = values.clone();
            let payload_parameters = payloads
                .iter()
                .map(|(_, value_type)| {
                    let declaration = ValueDeclaration {
                        qualifications: value_type.qualifications,
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type: value_type.scalar_type,
                    };
                    dispatch_namespace.push(declaration);
                    Ok(declaration)
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            let selected_block = block_id(allocate_dense(next_block)?);
            // A fallback edge exists only for declared cases outside the
            // selected one; a single-case sum leaves the source `_` arm
            // unreachable, so its continuation block must not be emitted.
            let has_fallback = cases.iter().any(|case| *case != *selected);
            let fallback_block = if has_fallback {
                Some(block_id(allocate_dense(next_block)?))
            } else {
                None
            };
            let when_true_arguments = dispatch_arguments(
                when_true_arguments,
                &dispatch_namespace,
                next_value,
                operations,
            )?;
            let when_false_arguments = if has_fallback {
                dispatch_arguments(when_false_arguments, &values, next_value, operations)?
            } else {
                Vec::new()
            };
            let when_true_erased_arguments = when_true_erased_arguments
                .iter()
                .map(|argument| {
                    crate::proofs::crash_routes::lowered_direct_scalar_term(
                        argument,
                        &dispatch_namespace,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let when_true_erased_proof_arguments = when_true_erased_proof_arguments
                .iter()
                .map(|term| {
                    crate::scalar_graph::scalar_contracts::lowered_proof_term(
                        term,
                        &dispatch_namespace,
                        &erased_formals,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (when_false_erased_arguments, when_false_erased_proof_arguments) = if has_fallback {
                (
                    when_false_erased_arguments
                        .iter()
                        .map(|argument| {
                            crate::proofs::crash_routes::lowered_direct_scalar_term(
                                argument,
                                &values,
                                &erased_formals,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    when_false_erased_proof_arguments
                        .iter()
                        .map(|term| {
                            crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                term,
                                &values,
                                &erased_formals,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                (Vec::new(), Vec::new())
            };
            blocks.push(Block {
                structural_parameters: Vec::new(),
                id: selected_block,
                parameters: payload_parameters,
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                operations: Vec::new(),
                terminator: Terminator::Jump {
                    structural_arguments: Vec::new(),
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: *targets
                        .get(*when_true_target)
                        .ok_or(LoweringError::Unsupported("call true target is absent"))?,
                    arguments: when_true_arguments,
                    erased_arguments: when_true_erased_arguments,
                    erased_proof_arguments: when_true_erased_proof_arguments,
                    cleanup_actions: Vec::new(),
                },
            });
            if let Some(fallback_block) = fallback_block {
                blocks.push(Block {
                    structural_parameters: Vec::new(),
                    id: fallback_block,
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        structural_arguments: Vec::new(),
                        edge: edge_id(allocate_dense(next_edge)?),
                        target: *targets
                            .get(*when_false_target)
                            .ok_or(LoweringError::Unsupported("call false target is absent"))?,
                        arguments: when_false_arguments,
                        erased_arguments: when_false_erased_arguments,
                        erased_proof_arguments: when_false_erased_proof_arguments,
                        cleanup_actions: Vec::new(),
                    },
                });
            }
            let mut case_edges = Vec::with_capacity(cases.len());
            for case in cases {
                case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                    edge: edge_id(allocate_dense(next_edge)?),
                    target: if *case == *selected {
                        selected_block
                    } else {
                        fallback_block.expect("non-selected case edge needs a fallback block")
                    },
                    case: *case,
                    payload_fields: if *case == *selected {
                        payloads.iter().map(|(field, _)| *field).collect()
                    } else {
                        Vec::new()
                    },
                    cleanup_actions: Vec::new(),
                });
            }
            Terminator::StructuralCase {
                source: *source,
                cases: case_edges,
            }
        }
        LoweredScalarBranchTerminator::CaseDispatchSplit {
            source,
            cases,
            armed,
            fallback_target,
            fallback_arguments,
            fallback_erased_arguments,
            fallback_erased_proof_arguments,
        } => {
            // Every armed case's edge binds its own payloads through a
            // parameter-only block, matching `Terminator::StructuralCase`'s
            // per-edge `payload_fields` roster.
            let mut case_edges = Vec::with_capacity(cases.len());
            let mut fallback_block = None;
            for case in cases {
                if let Some(arm) = armed.iter().find(|arm| arm.case == *case) {
                    let mut dispatch_namespace = values.clone();
                    let mut payload_parameters = Vec::with_capacity(arm.payloads.len());
                    for (_, value_type) in &arm.payloads {
                        let declaration = ValueDeclaration {
                            qualifications: value_type.qualifications,
                            id: value_id(allocate_dense(next_value)?),
                            scalar_type: value_type.scalar_type,
                        };
                        dispatch_namespace.push(declaration);
                        payload_parameters.push(declaration);
                    }
                    let selected_block = block_id(allocate_dense(next_block)?);
                    let arguments = dispatch_arguments(
                        &arm.arguments,
                        &dispatch_namespace,
                        next_value,
                        operations,
                    )?;
                    let erased_arguments = arm
                        .erased_arguments
                        .iter()
                        .map(|argument| {
                            crate::proofs::crash_routes::lowered_direct_scalar_term(
                                argument,
                                &dispatch_namespace,
                                &erased_formals,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let erased_proof_arguments = arm
                        .erased_proof_arguments
                        .iter()
                        .map(|term| {
                            crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                term,
                                &dispatch_namespace,
                                &erased_formals,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    blocks.push(Block {
                        structural_parameters: Vec::new(),
                        id: selected_block,
                        parameters: payload_parameters,
                        erased_scalar_formals: Vec::new(),
                        erased_proof_formals: Vec::new(),
                        operations: Vec::new(),
                        terminator: Terminator::Jump {
                            structural_arguments: Vec::new(),
                            edge: edge_id(allocate_dense(next_edge)?),
                            target: *targets.get(arm.target).ok_or(LoweringError::Unsupported(
                                "call case arm target is absent",
                            ))?,
                            arguments,
                            erased_arguments,
                            erased_proof_arguments,
                            cleanup_actions: Vec::new(),
                        },
                    });
                    case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                        edge: edge_id(allocate_dense(next_edge)?),
                        target: selected_block,
                        case: *case,
                        payload_fields: arm.payloads.iter().map(|(field, _)| *field).collect(),
                        cleanup_actions: Vec::new(),
                    });
                } else {
                    if fallback_block.is_none() {
                        let block = block_id(allocate_dense(next_block)?);
                        let arguments = dispatch_arguments(
                            fallback_arguments,
                            &values,
                            next_value,
                            operations,
                        )?;
                        let erased_arguments = fallback_erased_arguments
                            .iter()
                            .map(|argument| {
                                crate::proofs::crash_routes::lowered_direct_scalar_term(
                                    argument,
                                    &values,
                                    &erased_formals,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let erased_proof_arguments = fallback_erased_proof_arguments
                            .iter()
                            .map(|term| {
                                crate::scalar_graph::scalar_contracts::lowered_proof_term(
                                    term,
                                    &values,
                                    &erased_formals,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        blocks.push(Block {
                            structural_parameters: Vec::new(),
                            id: block,
                            parameters: Vec::new(),
                            erased_scalar_formals: Vec::new(),
                            erased_proof_formals: Vec::new(),
                            operations: Vec::new(),
                            terminator: Terminator::Jump {
                                structural_arguments: Vec::new(),
                                edge: edge_id(allocate_dense(next_edge)?),
                                target: *targets.get(*fallback_target).ok_or(
                                    LoweringError::Unsupported(
                                        "call case fallback target is absent",
                                    ),
                                )?,
                                arguments,
                                erased_arguments,
                                erased_proof_arguments,
                                cleanup_actions: Vec::new(),
                            },
                        });
                        fallback_block = Some(block);
                    }
                    case_edges.push(terminal_psi::StructuralCaseSuccessorEdge {
                        edge: edge_id(allocate_dense(next_edge)?),
                        target: fallback_block.expect("armed-edge loop emitted fallback"),
                        case: *case,
                        payload_fields: Vec::new(),
                        cleanup_actions: Vec::new(),
                    });
                }
            }
            Terminator::StructuralCase {
                source: *source,
                cases: case_edges,
            }
        }
        _ => {
            return unsupported(
                "call operand computation cannot return from its enclosing machine",
            );
        }
    };
    blocks.push(Block {
        structural_parameters: Vec::new(),
        id: block,
        parameters: block_parameters,
        erased_scalar_formals: erased_formals,
        erased_proof_formals: erased_proof_formal_declarations(source_erased_proof_formals),
        operations: operations[operation_start..].to_vec(),
        terminator,
    });
    Ok(())
}

/// Dispatch edge arguments evaluate in the block that stages them: the
/// namespace is the dispatch state's values plus the edge's bound payloads.
fn dispatch_arguments(
    expressions: &[LoweredDirectExpression],
    namespace: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Vec<ValueId>, LoweringError> {
    expressions
        .iter()
        .map(|expression| {
            validate_direct_parameter_types(
                expression,
                &namespace
                    .iter()
                    .map(|value| value.scalar_type)
                    .collect::<Vec<_>>(),
            )?;
            if direct_expression_contains_short_circuit(expression) {
                return unsupported("call computation transfer retains unexpanded Boolean control");
            }
            Ok(emit_direct_expression(
                expression, namespace, next_value, operations,
            ))
        })
        .collect()
}
