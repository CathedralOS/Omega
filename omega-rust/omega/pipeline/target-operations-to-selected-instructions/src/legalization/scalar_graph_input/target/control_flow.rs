//! Exact control block replay into the scalar graph, without executable target rows.
use super::*;
use target_operations::{TargetControlGraph, TargetControlSuccessor, TargetControlTerminator};
#[cfg(test)]
mod return_cleanup_tests;
#[cfg(test)]
mod scalar_return_tests;
pub(in crate::legalization::scalar_graph_input) mod sources;
#[cfg(test)]
mod structural_case_tests;
mod structural_cases;
#[cfg(test)]
mod tests;

pub(super) fn validate(
    function: &TargetFunction,
    graph: &TargetControlGraph,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if function.attachment != optimized.attachment
        || graph.structural_types != plan.structural_types
        || graph.structural_types != unit.structural_types
        || graph.entry != optimized.entry
        || graph.blocks.len() != optimized.blocks.len()
    {
        return Err(invalid);
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
                .ok_or(invalid.clone())?;
            if graph.call_plan != expected
                || graph.scalar_parameters != abi.scalar_parameters
                || graph.parameters != abi.structural_parameters
                || (!super::super::unobserved_owned::body(optimized)
                    && graph
                        .blocks
                        .iter()
                        .any(|block| !block.structural_parameters.is_empty()))
            {
                return Err(invalid);
            }
        }
        AbstractFunctionResult::Scalar(_) => {
            let expected =
                super::super::header::function_abi(native.target, function, abstracted, optimized)?;
            let abi = function.scalar_abi.as_ref().ok_or(invalid.clone())?;
            if graph.call_plan != expected
                || graph.scalar_parameters != abi.parameters
                || !graph.parameters.is_empty()
                || !super::super::primitive_locals::roster(optimized)
                || graph
                    .blocks
                    .iter()
                    .any(|block| !block.structural_parameters.is_empty())
            {
                return Err(invalid);
            }
        }
        _ => return Err(invalid),
    }
    for (block, source) in graph.blocks.iter().zip(&optimized.blocks) {
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
            || source.nodes.len() != block.operations.len() + 1
        {
            return Err(invalid);
        }
        let mut available = sources::available(graph, optimized, block.block);
        for (operation, node) in block.operations.iter().zip(&source.nodes) {
            // Returns belong only to the terminator, never an ordinary row.
            if matches!(operation, TargetUnitOperation::Return { .. }) {
                return Err(invalid);
            }
            super::unit::validate_operation(
                function,
                operation,
                &node.operation,
                &graph.scalar_parameters,
                &graph.parameters,
                &mut available,
                optimized,
                native,
                plan,
                unit,
            )?;
        }
        let source_terminator = &source.nodes.last().ok_or(invalid.clone())?.operation;
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
                    returned_claims,
                    trivial_affine_locals,
                    trivial_affine_discards,
                },
            ) => {
                psi_edge == expected_edge
                    && cleanup_actions.is_empty()
                    && returned_claims.is_empty()
                    && trivial_affine_locals.is_empty()
                    && trivial_affine_discards.is_empty()
                    && match source {
                        target_operations::TargetStructuralReturnSource::Parameter(parameter) => {
                            optimized.structural_parameters.iter().any(|semantic| {
                                semantic.place == *expected_source
                            && semantic.access == terminal_psi::StructuralAccess::Owned
                            && (semantic.multiplicity
                                == terminal_psi::StructuralMultiplicity::Affine
                                || semantic.multiplicity
                                    == terminal_psi::StructuralMultiplicity::Unrestricted
                                    && crate::structural_reference_input::primitive_array_shape(
                                        semantic.structural_type,
                                        &plan.structural_types,
                                    )
                                    .is_some())
                            && !semantic.is_self
                            && semantic.qualifications.is_empty()
                            && semantic.projected_qualifications.is_empty()
                            && optimized.result.structural().is_some_and(|result| {
                                result.structural_type == semantic.structural_type
                                    && result.multiplicity == semantic.multiplicity
                                    && result.qualifications.is_empty()
                                    && result.projected_qualifications.is_empty()
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
                                    .is_some_and(|result| result.shape == parameter.shape)
                        }
                        target_operations::TargetStructuralReturnSource::Home(source) => {
                            super::super::aggregate_results::result_home(
                                optimized,
                                *expected_source,
                                plan,
                            )
                            .is_ok_and(|expected| expected == *source)
                                && graph.blocks.iter().any(|producer| {
                                    (producer.block == block.block
                                        || sources::dominates(
                                            optimized,
                                            producer.block,
                                            block.block,
                                        ))
                                        && producer.operations.iter().any(|operation| {
                                            match operation {
                                                TargetUnitOperation::EstablishScalarArray {
                                                    result_home,
                                                    ..
                                                }
                                                | TargetUnitOperation::EstablishScalarCase {
                                                    result_home,
                                                    ..
                                                }
                                                | TargetUnitOperation::StructuralResultCall {
                                                    result_home: Some(result_home),
                                                    ..
                                                } => result_home == source,
                                                _ => false,
                                            }
                                        })
                                })
                        }
                    }
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
                        || (super::super::unobserved_owned::body(optimized)
                            && super::super::unobserved_owned::cleanup(optimized, cleanup_actions)))
                    && available.iter().any(|(identity, source)| {
                        identity == value && source.scalar_type() == *expected_type
                    })
                    && {
                        let checker = Checker {
                            function,
                            available: Some(&available),
                            optimized,
                            native,
                            plan,
                            unit,
                        };
                        match expression {
                            TargetScalarExpression::Integer {
                                scalar_type,
                                expression,
                            } => {
                                *expected_type == ScalarType::Integer(*scalar_type)
                                    && checker.expression(expression, *value, &[])
                            }
                            TargetScalarExpression::Boolean(expression) => {
                                *expected_type == ScalarType::Boolean
                                    && checker.boolean(expression, *value, &[])
                            }
                        }
                    }
            }
            (
                TargetControlTerminator::StructuralCase { source, cases },
                AbstractOperation::StructuralCase {
                    source: expected_source,
                    cases: expected_cases,
                },
            ) => structural_cases::matches(
                graph,
                optimized,
                block.block,
                source,
                cases,
                *expected_source,
                expected_cases,
            ),
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
                        || super::super::aggregate_results::cleanup(optimized, cleanup)
                        || (super::super::unobserved_owned::body(optimized)
                            && super::super::unobserved_owned::cleanup(optimized, cleanup)))
            }
            (
                TargetControlTerminator::Jump { successor },
                AbstractOperation::Jump {
                    psi_edge,
                    target,
                    bindings,
                    structural_bindings,
                    trivial_affine_discards,
                    residual_affine_discards,
                },
            ) => {
                successor.psi_edge == *psi_edge
                    && successor.target == *target
                    && successor.bindings == *bindings
                    && successor.structural_bindings == *structural_bindings
                    && successor.cleanup_actions.is_empty()
                    && trivial_affine_discards.is_empty()
                    && residual_affine_discards.is_empty()
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
                        native,
                        plan,
                        unit,
                    })
                    .boolean(condition, *expected, &[])
                    && successor_matches(when_true, expected_true)
                    && successor_matches(when_false, expected_false)
            }
            _ => false,
        };
        if !matches {
            return Err(invalid);
        }
    }
    Ok(())
}

fn successor_matches(
    target: &TargetControlSuccessor,
    source: &abstract_operations::AbstractSuccessor,
) -> bool {
    target.psi_edge == source.psi_edge
        && target.target == source.target
        && target.bindings == source.bindings
        && target.structural_bindings == source.structural_bindings
        && source.trivial_affine_discards.is_empty()
        && target.cleanup_actions.is_empty()
}
