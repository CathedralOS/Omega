//! Optimizer module role: executable entrance. Reconstructs one scalar leaf through ordered scalar operations and catalogued compatibility forms, then seals its return envelope.

mod context;
mod direct_exact_binary;
mod entry_parameter;
mod fuel;
mod immediate;
mod operation_projection;
mod return_projection;
mod sequence;
mod widened_exact_binary;

use super::shared::*;
use context::{DerivedValue, LeafContext};
pub(in crate::legalization::source) use fuel::{exact_edge_fuel, exact_operation_fuel};
pub(super) use operation_projection::source_operations;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[optimization_unit::OptimizationNode],
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    temporaries: [LegalizedTemporaryId; 2],
    ordered_sequence: bool,
) -> Result<SourceLeaf, LegalizationError> {
    if nodes.len() != abstract_operations.len()
        || nodes
            .iter()
            .zip(abstract_operations)
            .any(|(node, operation)| node.operation != *operation)
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let TargetIntegerControl::Return {
        psi_return_edge,
        source_value,
        expression,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    let u64_integer_type =
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
    let context = LeafContext {
        function,
        arm_edge,
        source_value: *source_value,
        nodes,
        abstracted,
        optimized,
        accepted_obligation_facts,
        temporaries,
        u64_integer_type,
        u64_type: ScalarType::Integer(u64_integer_type),
    };

    let (return_node, value) = if ordered_sequence {
        sequence::derive(&context, expression)?
    } else {
        match expression {
            TargetIntegerExpression::Immediate { .. } => immediate::derive(&context, expression)?,
            TargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            } => entry_parameter::derive(&context, expression, *parameter_index, location)?,
            TargetIntegerExpression::IntegerWiden { .. } => {
                widened_exact_binary::derive_expression(&context, expression)?
            }
            expression @ TargetIntegerExpression::ExactAdd { .. } => {
                direct_exact_binary::derive_add(&context, expression)?
            }
            expression @ TargetIntegerExpression::ExactSubtract { .. } => {
                direct_exact_binary::derive_subtract(&context, expression)?
            }
            _ => return Err(Error::UnsupportedSourceShape { function }),
        }
    };
    return_projection::finalize(&context, return_node, *psi_return_edge, value)
}
