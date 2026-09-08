//! Scalar source availability is indexed by source CFG dominance, not target block order.
use super::*;
use target_operations::TargetUnitScalarArgumentSource as Source;

pub(super) fn available(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    block: BlockId,
) -> Vec<(ValueId, Source)> {
    let mut result = optimized
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.value,
                Source::Parameter {
                    parameter_index: index as u32,
                    source_value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            )
        })
        .collect::<Vec<_>>();
    for candidate in &graph.blocks {
        if candidate.block == block || dominates(optimized, candidate.block, block) {
            result.extend(candidate.parameters.iter().map(|parameter| {
                (
                    parameter.value,
                    Source::BlockParameter(target_operations::TargetScalarBlockValue {
                        block: candidate.block,
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    }),
                )
            }));
            if candidate.block != block {
                result.extend(candidate.operations.iter().filter_map(definition));
            }
        }
    }
    result
}

// Candidate rows are independently replayed against their exact source nodes by
// the caller; collecting their descriptions does not authorize a producer or use.
fn definition(operation: &TargetUnitOperation) -> Option<(ValueId, Source)> {
    match operation {
        TargetUnitOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => Some((
            *result,
            Source::BooleanImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                value: *value,
            },
        )),
        TargetUnitOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => Some((
            *result,
            Source::IntegerImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: *scalar_type,
                value: *value,
            },
        )),
        TargetUnitOperation::ScalarDefinition { result_home, .. }
        | TargetUnitOperation::ScalarCall { result_home, .. } => {
            Some((result_home.source_value, Source::Home(*result_home)))
        }
        _ => None,
    }
}

pub(super) fn dominates(
    function: &PsiOptimizationFunction,
    candidate: BlockId,
    block: BlockId,
) -> bool {
    let mut pending = vec![function.entry];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if current == candidate || visited.contains(&current) {
            continue;
        }
        if current == block {
            return false;
        }
        visited.push(current);
        if let Some(source) = function.blocks.iter().find(|source| source.id == current) {
            pending.extend(
                source
                    .nodes
                    .iter()
                    .flat_map(|node| &node.successors)
                    .map(|edge| edge.target),
            );
        }
    }
    true
}
