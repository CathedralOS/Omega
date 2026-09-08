//! Exact Unit block replay into the existing scalar graph, without executable target rows.
use super::*;
use target_operations::{TargetUnitGraph, TargetUnitSuccessor, TargetUnitTerminator};
#[cfg(test)]
mod return_cleanup_tests;
mod sources;
#[cfg(test)]
mod structural_case_tests;
mod structural_cases;
#[cfg(test)]
mod tests;

pub(super) fn validate(
    function: &TargetFunction,
    graph: &TargetUnitGraph,
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
        || optimized.result != AbstractFunctionResult::Unit
    {
        return Err(invalid);
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
                TargetUnitTerminator::StructuralCase { source, cases },
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
                TargetUnitTerminator::Return {
                    psi_edge,
                    cleanup_actions,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: expected,
                    cleanup_actions: cleanup,
                },
            ) => {
                psi_edge == expected
                    && cleanup_actions == cleanup
                    && (cleanup.is_empty() || super::super::read_byte::cleanup(optimized, cleanup))
            }
            (
                TargetUnitTerminator::Jump { successor },
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
                TargetUnitTerminator::Conditional {
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
    target: &TargetUnitSuccessor,
    source: &abstract_operations::AbstractSuccessor,
) -> bool {
    target.psi_edge == source.psi_edge
        && target.target == source.target
        && target.bindings == source.bindings
        && target.structural_bindings == source.structural_bindings
        && source.trivial_affine_discards.is_empty()
        && target.cleanup_actions.is_empty()
}
