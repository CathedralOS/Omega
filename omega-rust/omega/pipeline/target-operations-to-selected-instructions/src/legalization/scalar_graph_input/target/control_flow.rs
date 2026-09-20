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
#[cfg(test)]
mod ieee_float_tests;
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
#[cfg(test)]
mod wrapping_remainder_tests;

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
        _ if super::super::aggregate_results::uses(optimized, plan) => {
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
            {
                return Err(invalid);
            }
        }
        AbstractFunctionResult::Scalar(_) => {
            let expected =
                super::super::header::function_abi(native.target, function, abstracted, optimized)?;
            // `scalar_abi` is only emitted for functions without descriptor
            // parameters; with them, the graph header is the sole signature
            // record and the roster rows carry the descriptor bindings.
            if let Some(abi) = &function.scalar_abi {
                if graph.call_plan != expected
                    || graph.scalar_parameters != abi.parameters
                    || !graph.dynamic_parameters.is_empty()
                {
                    return Err(invalid);
                }
            } else if graph.call_plan != expected || graph.dynamic_parameters.is_empty() {
                return Err(invalid);
            }
            if !graph.parameters.is_empty()
                || !(super::super::primitive_locals::roster(optimized)
                    || super::super::literals::roster(optimized))
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
    // Reference custody is replayed independently alongside the row checks:
    // each block enters with its dominator's exit state (ingress seeding at
    // the entry block), and every operation row validates against the custody
    // state it leaves suspended.
    let block_entries = super::super::reference_custody::block_entry_states(optimized, plan, unit)?;
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
            || source
                .nodes
                .iter()
                .filter(|node| !super::super::indirect_calls::is_descriptor_declaration(node))
                .count()
                != block.operations.len() + 1
        {
            return Err(invalid);
        }
        let source_nodes: Vec<_> = source
            .nodes
            .iter()
            .filter(|node| !super::super::indirect_calls::is_descriptor_declaration(node))
            .collect();
        let mut available = sources::available(graph, optimized, block.block);
        let mut custody = block_entries
            .get(&block.block)
            .cloned()
            .ok_or(invalid.clone())?;
        for (operation, node) in block.operations.iter().zip(&source_nodes) {
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
                &custody,
                optimized,
                native,
                plan,
                unit,
            )?;
            super::super::reference_custody::apply(
                &mut custody,
                &node.operation,
                optimized,
                plan,
                unit,
            )?;
        }
        let source_terminator = &source.nodes.last().ok_or(invalid.clone())?.operation;
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
                    returned_claims,
                    trivial_affine_locals,
                    trivial_affine_discards,
                },
            ) => {
                psi_edge == expected_edge
                    // No destructor instructions are needed for these plain homes;
                    // retain and replay their exact ordered death-edge disposition.
                    && edge_cleanup_matches(
                        optimized,
                        cleanup_actions,
                        trivial_affine_discards,
                        &[],
                    )
                    && returned_claims.is_empty()
                    && trivial_affine_locals.is_empty()
                    // A reference-bearing result must still sit at its declared
                    // carrier paths with each leaf rooted at a formal origin.
                    && optimized.result.structural().is_none_or(|result| {
                        !super::super::reference_custody::contains_reference(
                            &plan.structural_types,
                            result.structural_type,
                        ) || super::super::reference_custody::return_custody(
                            &custody,
                            optimized,
                            &plan.structural_types,
                            *expected_source,
                            result,
                        )
                        .is_ok()
                    })
                    && match source {
                        target_operations::TargetStructuralReturnSource::Parameter(parameter) => {
                            optimized.structural_parameters.iter().any(|semantic| {
                                semantic.place == *expected_source
                            && semantic.access == terminal_psi::StructuralAccess::Owned
                            && semantic.multiplicity
                                != terminal_psi::StructuralMultiplicity::Linear
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
                        || super::super::aggregate_results::cleanup(optimized, cleanup_actions)
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
                        };
                        match expression {
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
                TargetControlTerminator::StructuralCase { source, cases },
                AbstractOperation::StructuralCase {
                    source: expected_source,
                    cases: expected_cases,
                },
            ) => structural_cases::matches(
                graph,
                optimized,
                plan,
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
                    && edge_cleanup_matches(
                        optimized,
                        &successor.cleanup_actions,
                        trivial_affine_discards,
                        residual_affine_discards,
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
                    })
                    .boolean_source(condition, *expected, &[])
                    && successor_matches(optimized, when_true, expected_true)
                    && successor_matches(optimized, when_false, expected_false)
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
    function: &PsiOptimizationFunction,
    target: &TargetControlSuccessor,
    source: &abstract_operations::AbstractSuccessor,
) -> bool {
    target.psi_edge == source.psi_edge
        && target.target == source.target
        && target.bindings == source.bindings
        && target.structural_bindings == source.structural_bindings
        && edge_cleanup_matches(
            function,
            &target.cleanup_actions,
            &source.trivial_affine_discards,
            &[],
        )
}

fn edge_cleanup_matches(
    function: &PsiOptimizationFunction,
    actions: &[terminal_psi::TerminalAffineCleanupAction],
    places: &[semantic_vocabulary::PlaceId],
    residuals: &[terminal_psi::StructuralAffineDiscard],
) -> bool {
    let expected = places
        .iter()
        .copied()
        .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
        .chain(
            residuals
                .iter()
                .cloned()
                .map(terminal_psi::TerminalAffineCleanupAction::DiscardResidual),
        )
        .collect::<Vec<_>>();
    actions == expected.as_slice() && super::super::aggregate_results::cleanup(function, actions)
}
