//! Exact control block replay into the scalar graph, without executable target rows.
use super::super::{AbstractFunctionResult, ScalarType};
use super::{
    AbstractFunction, AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction,
    PsiOptimizationUnit, TargetFunction, TargetOperationPlan, TargetScalarExpression,
    TargetUnitOperation,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target::Checker;
use target_operations::{TargetControlGraph, TargetControlSuccessor, TargetControlTerminator};
pub(in crate::legalization::scalar_graph_input) mod sources;
mod structural_cases;

pub(super) fn validate(
    function: &TargetFunction,
    graph: &TargetControlGraph,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<super::super::BlockEntryCustody, LegalizationError> {
    if (function.attachment != optimized.attachment
        || function.nominal_cleanup_subject != optimized.nominal_cleanup_subject)
        || graph.structural_types != plan.structural_types
        || graph.structural_types != unit.structural_types
        || graph.entry != optimized.entry
        || graph.blocks.len() != optimized.blocks.len()
    {
        return Err(LegalizationError::custody());
    }
    match optimized.result {
        _ if super::super::aggregate_results::uses(optimized) => {
            super::super::aggregate_results::header(
                function,
                abstracted,
                optimized,
                native.target,
                plan,
            )?;
        }
        AbstractFunctionResult::Unit => {}
        AbstractFunctionResult::Scalar(_) if function.mixed_structural_scalar_abi.is_some() => {
            let expected = super::super::byte_views::validate(
                function,
                abstracted,
                optimized,
                native.target,
                plan,
            )?;
            let abi = function
                .mixed_structural_scalar_abi
                .as_ref()
                .ok_or(LegalizationError::custody())?;
            if graph.call_plan != expected
                || graph.scalar_parameters != abi.scalar_parameters
                || graph.parameters != abi.structural_parameters
            {
                return Err(LegalizationError::custody());
            }
        }
        AbstractFunctionResult::Scalar(_) => {
            let expected =
                super::super::header::function_abi(native.target, function, abstracted, optimized)?;
            let Some(abi) = &function.scalar_abi else {
                return Err(LegalizationError::custody());
            };
            if graph.call_plan != expected || graph.scalar_parameters != abi.parameters {
                return Err(LegalizationError::custody());
            }
            if !graph.parameters.is_empty()
                || graph
                    .blocks
                    .iter()
                    .any(|block| !block.structural_parameters.is_empty())
            {
                return Err(LegalizationError::custody());
            }
        }
        _ => return Err(LegalizationError::custody()),
    }
    // Reference custody is replayed independently alongside the row checks:
    // each block enters with its dominator's exit state (ingress seeding at
    // the entry block), and every operation row validates against the custody
    // state it leaves suspended.
    let block_entries =
        super::super::reference_custody::block_entry_states(optimized, native, plan, unit)?;
    let dominators = sources::SourceDominators::new(optimized);
    for (block, source) in graph.blocks.iter().zip(&optimized.blocks) {
        let body_len = block
            .operations
            .iter()
            .position(|row| matches!(row, TargetUnitOperation::CleanupCall { .. }))
            .unwrap_or(block.operations.len());
        let (body, cleanup_calls) = block.operations.split_at(body_len);

        if block.block != source.id
            || block.structural_parameters != source.structural_parameters
            || block.parameters.len() != source.parameters.len()
            || block
                .parameters
                .iter()
                .zip(&source.parameters)
                .any(|(actual, expected)| {
                    actual.value != expected.value || actual.scalar_type != expected.scalar_type
                })
            || (source.id == optimized.entry && !source.parameters.is_empty())
            || source.nodes.len() != body.len() + 1
        {
            return Err(LegalizationError::custody());
        }
        let source_nodes: Vec<_> = source.nodes.iter().collect();
        let mut available = sources::available(graph, optimized, &dominators, block.block);
        let mut custody = block_entries
            .get(&block.block)
            .cloned()
            .ok_or(LegalizationError::custody())?;
        for (node_index, (operation, node)) in body.iter().zip(&source_nodes).enumerate() {
            custody.set_position(block.block, node_index)?;
            // Returns belong only to the terminator, never an ordinary row.
            if matches!(operation, TargetUnitOperation::Return { .. }) {
                return Err(LegalizationError::custody());
            }
            super::unit::validate_operation(
                function,
                operation,
                &node.operation,
                &graph.scalar_parameters,
                &graph.parameters,
                &mut available,
                &custody,
                optimized,
                native,
                plan,
                unit,
            )?;
            super::super::reference_custody::apply(
                &mut custody,
                node,
                optimized,
                native,
                plan,
                unit,
            )?;
        }
        custody.set_position(block.block, body.len())?;
        let source_terminator = &source
            .nodes
            .last()
            .ok_or(LegalizationError::custody())?
            .operation;
        super::super::cleanup_calls::validate_target(
            cleanup_calls,
            source_terminator,
            optimized,
            native,
            plan,
            unit,
        )?;
        // The terminator's edge-local discards move custody after every row,
        // in the same order the lowering's cleanup pass commits them.
        super::super::reference_custody::apply_terminator(
            &mut custody,
            source_terminator,
            optimized,
            plan,
        )?;
        let matches = match (&block.terminator, source_terminator) {
            (
                TargetControlTerminator::ReturnStructural {
                    psi_edge,
                    source,
                    cleanup_actions,
                },
                AbstractOperation::ReturnStructural {
                    psi_edge: expected_edge,
                    source: expected_source,
                    trivial_affine_locals,
                    cleanup_actions: expected_cleanup,
                    ..
                },
            ) => {
                // The selected return transaction authenticated and moved its
                // actual loan roster before committing these cleanup actions.
                psi_edge == expected_edge
                    && cleanup_actions == expected_cleanup
                    && trivial_affine_locals.is_empty()
                    && match source {
                        target_operations::TargetStructuralReturnSource::SharedDescriptor { place } => {
                            place == expected_source
                                && super::super::byte_views::descriptor_return(
                                    optimized, *place, &plan.structural_types,
                                )
                                && graph.call_plan.result.as_ref().is_some_and(|result|
                                    result.shape == calling_conventions::ValueShape::integer(16, 8))
                                && (optimized.structural_parameter(*place).is_some()
                                    || optimized.blocks.iter().any(|producer| {
                                        (producer.id == block.block || sources::dominates(
                                            optimized, producer.id, block.block))
                                            && (producer.structural_parameter(*place).is_some()
                                                || producer.nodes.iter().any(|node|
                                                    node.operation.structural_result().is_some_and(
                                                        |(_, result)| result.place == *place)
                                                    || matches!(&node.operation,
                                                        AbstractOperation::EstablishByteSequenceLiteral {
                                                            place: declaration, ..
                                                        } if declaration.id == *place)))
                                    }))
                        }
                        target_operations::TargetStructuralReturnSource::Parameter(parameter) => {
                            optimized.structural_parameters.iter().any(|semantic| {
                                semantic.place == *expected_source
                            && semantic.access == terminal_psi::StructuralAccess::Owned
                            && optimized.result.structural().is_some_and(|result| {
                                result.structural_type == semantic.structural_type
                                    && result.multiplicity == semantic.multiplicity
                            })
                            }) && parameter.place == *expected_source
                                && graph
                                    .parameters
                                    .iter()
                                    .any(|expected| expected == parameter)
                                && graph
                                    .call_plan
                                    .result
                                    .as_ref()
                                    .is_some_and(|result| {
                                        result.shape == parameter.shape
                                    })
                        }
                        target_operations::TargetStructuralReturnSource::Home(source) => {
                            super::super::aggregate_results::result_home(
                                optimized,
                                *expected_source,
                                plan,
                            )
                            .is_ok_and(|expected| expected == *source)
                                && structural_cases::home_available(
                                    graph,
                                    optimized,
                                    plan,
                                    block.block,
                                    source,
                                    *expected_source,
                                )
                        }
                    }
            }
            (
                TargetControlTerminator::ExternalCompletion {
                    psi_edge,
                    boundary,
                    kind,
                    arguments,
                    binding,
                    structural_arguments,
                    requirement_obligations,
                    frontier_lower_bound,
                },
                AbstractOperation::ExternalCompletion {
                    psi_edge: expected_edge,
                    boundary: expected_boundary,
                    kind: expected_kind,
                    arguments: expected_arguments,
                    structural_arguments: expected_structural,
                    requirement_obligations: expected_obligations,
                    frontier_lower_bound: expected_frontier,
                },
            ) => {
                let declaration = plan
                    .boundary_machine(*boundary)
                    .ok_or(LegalizationError::custody())?;
                let parameters = declaration
                    .scalar_parameters
                    .iter()
                    .map(|scalar_type| {
                        super::super::scalar_shape(*scalar_type).ok_or(LegalizationError::custody())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let signature = calling_conventions::CallSignature {
                    parameters,
                    result: None,
                };
                let expected_plan = match &binding.realization {
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                        foreign,
                    ) => calling_conventions::validate_boundary_entry_plan(
                        foreign.boundary_entry_plan.clone(),
                        &signature,
                    )
                    .map_err(|_| LegalizationError::custody())?
                    .plan()
                    .call
                    .clone(),
                    _ => calling_conventions::evaluate_call_plan(
                        calling_conventions::CallingPolicy::native_for_target(native.target),
                        &signature,
                    )
                    .map_err(|_| LegalizationError::custody())?,
                };
                psi_edge == expected_edge
                    && boundary == expected_boundary
                    && kind == expected_kind
                    && structural_arguments == expected_structural
                    && requirement_obligations == expected_obligations
                    && frontier_lower_bound == expected_frontier
                    && binding.boundary == *boundary
                    && declaration.completion
                        == semantic_vocabulary::BoundaryCompletion::External(*kind)
                    && declaration.result.is_unit()
                    && arguments.len() == expected_arguments.len()
                    && arguments.len() == expected_plan.parameters.len()
                    && structural_arguments.len() == declaration.structural_parameters.len()
                    && arguments
                        .iter()
                        .zip(expected_arguments)
                        .zip(&expected_plan.parameters)
                        .zip(&declaration.scalar_parameters)
                        .enumerate()
                        .all(
                            |(position, (((argument, value), placement), scalar_type))| {
                                argument.parameter_index as usize == position
                                    && argument.placement == *placement
                                    && argument.source.source_value() == *value
                                    && argument.source.scalar_type() == *scalar_type
                                    && available
                                        .iter()
                                        .rev()
                                        .find(|(candidate, _)| candidate == value)
                                        .is_some_and(|(_, source)| source == &argument.source)
                            },
                        )
            }
            (
                TargetControlTerminator::Crash {
                    psi_edge,
                    cause,
                    site_guard,
                    frontier_lower_bound,
                },
                AbstractOperation::Crash {
                    psi_edge: expected_edge,
                    cause: expected_cause,
                    site_guard: expected_guard,
                    frontier_lower_bound: expected_frontier,
                },
            ) => {
                psi_edge == expected_edge
                    && cause == expected_cause
                    && site_guard == expected_guard
                    && frontier_lower_bound == expected_frontier
            }
            (
                TargetControlTerminator::ReturnScalar {
                    psi_edge,
                    source_value,
                    expression,
                    cleanup_actions,
                },
                AbstractOperation::Return {
                    psi_edge: expected_edge,
                    result,
                    value,
                    scalar_type: expected_type,
                    cleanup_actions: expected_cleanup,
                },
            ) => {
                matches!(optimized.result, AbstractFunctionResult::Scalar(declaration)
                    if declaration.value == *result && declaration.scalar_type == *expected_type)
                    && psi_edge == expected_edge
                    && source_value == value
                    && cleanup_actions == expected_cleanup
                    && (cleanup_actions.is_empty()
                        || super::super::aggregate_results::cleanup(
                            optimized,
                            cleanup_actions,
                            &plan.structural_types,
                        ))
                    && available.iter().any(|(identity, source)| {
                        identity == value && source.scalar_type() == *expected_type
                    })
                    && {
                        let checker = Checker {
                            function,
                            available: Some(&available),
                            optimized,
                            types: unit.structural_types.as_slice(),
                        };
                        match expression {
                            TargetScalarExpression::IeeeFloatElementViewRead { .. } => false,
                            TargetScalarExpression::IeeeFloat(source) => {
                                matches!(expected_type, ScalarType::IeeeFloat(_))
                                    && source.scalar_type() == *expected_type
                                    && source.source_value() == *value
                                    && available
                                        .iter()
                                        .filter(|(candidate, _)| candidate == value)
                                        .map(|(_, candidate)| candidate)
                                        .eq(std::iter::once(source))
                            }
                            TargetScalarExpression::Integer {
                                scalar_type,
                                expression,
                            } => {
                                *expected_type == ScalarType::Integer(*scalar_type)
                                    && checker.integer_source(expression, *value, &[])
                            }
                            TargetScalarExpression::Boolean(expression) => {
                                *expected_type == ScalarType::Boolean
                                    && checker.boolean_source(expression, *value, &[])
                            }
                        }
                    }
            }
            (
                TargetControlTerminator::StructuralCase {
                    source,
                    path,
                    byte_offset,
                    layout,
                    cases,
                },
                AbstractOperation::StructuralCase {
                    source: expected_source,
                    path: expected_path,
                    cases: expected_cases,
                },
            ) => {
                path == expected_path
                    && structural_cases::matches(
                        graph,
                        optimized,
                        plan,
                        block.block,
                        source,
                        path,
                        *byte_offset,
                        layout,
                        cases,
                        *expected_source,
                        expected_cases,
                    )
            }
            (
                TargetControlTerminator::Return {
                    psi_edge,
                    cleanup_actions,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: expected,
                    cleanup_actions: cleanup,
                },
            ) => {
                optimized.result == AbstractFunctionResult::Unit
                    && psi_edge == expected
                    && cleanup_actions == cleanup
                    && (cleanup.is_empty()
                        || super::super::read_byte::cleanup(optimized, cleanup)
                        || super::super::aggregate_results::cleanup(
                            optimized,
                            cleanup,
                            &plan.structural_types,
                        ))
            }
            (
                TargetControlTerminator::Jump { successor },
                AbstractOperation::Jump {
                    psi_edge,
                    target,
                    bindings,
                    structural_bindings,
                    cleanup_actions,
                },
            ) => {
                successor.psi_edge == *psi_edge
                    && successor.target == *target
                    && successor.bindings == *bindings
                    && successor.structural_bindings == *structural_bindings
                    && super::super::address_joins::edge_bindings(
                        optimized,
                        block.block,
                        *target,
                        structural_bindings,
                        plan,
                    )
                    && edge_cleanup_matches(
                        optimized,
                        &successor.cleanup_actions,
                        cleanup_actions,
                        &plan.structural_types,
                    )
            }
            (
                TargetControlTerminator::Conditional {
                    condition_source,
                    condition,
                    when_true,
                    when_false,
                },
                AbstractOperation::Conditional {
                    condition: expected,
                    when_true: expected_true,
                    when_false: expected_false,
                },
            ) => {
                condition_source == expected
                    && (Checker {
                        function,
                        available: Some(&available),
                        optimized,
                        types: unit.structural_types.as_slice(),
                    })
                    .boolean_source(condition, *expected, &[])
                    && successor_matches(optimized, block.block, plan, when_true, expected_true)
                    && successor_matches(optimized, block.block, plan, when_false, expected_false)
            }
            _ => false,
        };
        if !matches {
            return Err(LegalizationError::custody());
        }
    }
    Ok(block_entries)
}

fn successor_matches(
    function: &PsiOptimizationFunction,
    block: semantic_vocabulary::BlockId,
    plan: &AbstractOperationPlan,
    target: &TargetControlSuccessor,
    source: &abstract_operations::AbstractSuccessor,
) -> bool {
    target.psi_edge == source.psi_edge
        && target.target == source.target
        && target.bindings == source.bindings
        && target.structural_bindings == source.structural_bindings
        && super::super::address_joins::edge_bindings(
            function,
            block,
            source.target,
            &source.structural_bindings,
            plan,
        )
        && edge_cleanup_matches(
            function,
            &target.cleanup_actions,
            &source.cleanup_actions,
            &plan.structural_types,
        )
}

fn edge_cleanup_matches(
    function: &PsiOptimizationFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
    expected: &[terminal_psi::TerminalAffineCleanupAction],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    actions == expected
        && super::super::aggregate_results::cleanup(function, actions, structural_types)
}
