//! Optimizer module role: executable entrance. Reconstructs one scalar leaf through the exact eight-form catalog, then seals its return envelope.

mod active_resident_exact_add_bridge_chain;
mod active_resident_exact_add_chain;
mod context;
mod direct_exact_binary;
mod entry_parameter;
mod exact_add;
mod fuel;
mod immediate;
mod operation_projection;
mod return_projection;
mod widened_exact_binary;

use super::shared::*;
use context::{DerivedValue, LeafContext};

pub(super) use exact_add::{
    is_active_resident_exact_add_bridge_chain, is_active_resident_exact_add_chain,
};
pub(super) use fuel::exact_edge_fuel;
pub(super) use operation_projection::source_operations;

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_leaf(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerControl,
    abstract_operations: &[AbstractOperation],
    nodes: &[omega_optimization_unit::OptimizationNode],
    abstracted: &omega_abstract_operations::AbstractFunction,
    optimized: &omega_optimization_unit::PsiOptimizationFunction,
    accepted_obligation_facts: &[AcceptedObligationFact],
    temporaries: [LegalizedTemporaryId; 2],
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
    let u64_integer_type = psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");
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

    let (return_node, value) = match expression {
        TargetIntegerExpression::Immediate { .. } => immediate::derive(&context, expression)?,
        TargetIntegerExpression::Parameter {
            parameter_index,
            location,
            ..
        } => entry_parameter::derive(&context, expression, *parameter_index, location)?,
        TargetIntegerExpression::IntegerWiden {
            psi_operation,
            source_type,
            operand,
        } => match operand.as_ref() {
            TargetIntegerExpression::ExactAdd { .. } => {
                widened_exact_binary::derive_add(&context, *psi_operation, *source_type, operand)?
            }
            TargetIntegerExpression::ExactSubtract { .. } => widened_exact_binary::derive_subtract(
                &context,
                *psi_operation,
                *source_type,
                operand,
            )?,
            _ => return Err(Error::UnsupportedSourceShape { function }),
        },
        expression @ TargetIntegerExpression::ExactAdd { .. } => {
            exact_add::derive(&context, expression)?
        }
        expression @ TargetIntegerExpression::ExactSubtract { .. } => {
            direct_exact_binary::derive_subtract(&context, expression)?
        }
        _ => return Err(Error::UnsupportedSourceShape { function }),
    };
    return_projection::finalize(&context, return_node, *psi_return_edge, value)
}
