//! Scalar source availability is indexed by source CFG dominance, not target block order.
use super::super::{BlockId, ValueId};
use super::{PsiOptimizationFunction, TargetControlGraph, TargetUnitOperation};
use target_operations::TargetUnitScalarArgumentSource as Source;

/// Resolve projection selectors at the call, excluding later definitions.
pub(in crate::legalization::scalar_graph_input) fn call_indices(
    native: &target_operations::TargetOperationPlan,
    caller: &PsiOptimizationFunction,
    call_operation: semantic_vocabulary::OperationId,
    elements: &[crate::structural_inputs::structural_reference_input::RuntimeElement],
) -> Result<Vec<target_operations::TargetStructuralRuntimeIndex>, crate::LegalizationError> {
    if elements.is_empty() {
        return Ok(Vec::new());
    }
    let function = native
        .functions
        .iter()
        .find(|function| function.machine == caller.machine)
        .ok_or(crate::LegalizationError::custody())?;
    let (call_block, call_position) = function
        .graph
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .position(|operation| {
                    matches!(operation, TargetUnitOperation::Call { psi_operation, .. }
                    if *psi_operation == call_operation)
                })
                .map(|position| (block, position))
        })
        .ok_or(crate::LegalizationError::custody())?;
    let mut sources = available(&function.graph, caller, call_block.block);
    sources.extend(
        call_block.operations[..call_position]
            .iter()
            .filter_map(definition),
    );
    elements
        .iter()
        .map(|element| {
            let (_, operand) = sources
                .iter()
                .find(|(value, _)| *value == element.index)
                .ok_or(crate::LegalizationError::custody())?;
            Ok(target_operations::TargetStructuralRuntimeIndex {
                operand: *operand,
                stride: element.stride,
            })
        })
        .collect()
}

pub(in crate::legalization::scalar_graph_input) fn available(
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
pub(in crate::legalization::scalar_graph_input) fn definition(
    operation: &TargetUnitOperation,
) -> Option<(ValueId, Source)> {
    match operation {
        TargetUnitOperation::IeeeFloatConstant {
            psi_operation,
            result,
            value,
        } => Some((
            *result,
            Source::IeeeFloatImmediate {
                defining_operation: *psi_operation,
                source_value: *result,
                value: *value,
            },
        )),
        TargetUnitOperation::PrimitiveScalarRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralScalarFieldRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralByteSequenceFieldRead {
            psi_operation,
            result,
            ..
        }
        | TargetUnitOperation::StructuralCaseMembership {
            psi_operation,
            result,
            ..
        } => Some((
            result.value,
            Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: result.value,
                scalar_type: result.scalar_type,
                shape: super::super::super::scalar_shape(result.scalar_type)?,
            }),
        )),
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
        TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
            psi_operation,
            result,
            format,
            ..
        } => Some((
            *result,
            Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: *result,
                scalar_type: semantic_vocabulary::ScalarType::IeeeFloat(*format),
                shape: super::super::super::scalar_shape(
                    semantic_vocabulary::ScalarType::IeeeFloat(*format),
                )?,
            }),
        )),
        TargetUnitOperation::IeeeFloatCompare { result_home, .. }
        | TargetUnitOperation::ScalarRepresentationView { result_home, .. }
        | TargetUnitOperation::FloatResultPolicy { result_home, .. }
        | TargetUnitOperation::NearestIeeeFloatArithmetic { result_home, .. }
        | TargetUnitOperation::ScalarDefinition { result_home, .. }
        | TargetUnitOperation::Call {
            result: target_operations::TargetCallResult::Scalar(result_home),
            ..
        }
        | TargetUnitOperation::NormalizedForeignCall {
            result: target_operations::TargetBoundaryResult::Scalar(result_home),
            ..
        } => Some((result_home.source_value, Source::Home(*result_home))),
        _ => None,
    }
}

pub(in crate::legalization::scalar_graph_input) fn dominates(
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
