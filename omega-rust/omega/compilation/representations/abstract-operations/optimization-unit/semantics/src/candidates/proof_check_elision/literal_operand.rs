//! The exact materialized integer literal one operand definition must name.
use abstract_operations::AbstractOperation as O;
use optimization_unit::{PsiOptimizationFunction, ValueDefinition, ValueDefinitionSite};
use semantic_vocabulary::{IntegerValue, OperationId, ScalarType, ValueId};

use crate::OptimizationUnitValidationError;

/// One materialized integer constant: the operation that published it, the
/// result and scalar type it published, and the literal value itself.
pub(super) struct MaterializedIntegerConstant {
    pub(super) operation: OperationId,
    pub(super) result: ValueId,
    pub(super) scalar_type: ScalarType,
    pub(super) value: IntegerValue,
}

/// The integer constant node `definition` lands on. A definition that names a
/// parameter, another value, or any other operation is a candidate operand-fact
/// mismatch, so a law's operand evidence is always read from one literal node.
pub(super) fn materialized_integer_constant(
    function: &PsiOptimizationFunction,
    definition: ValueDefinition,
) -> Result<MaterializedIntegerConstant, OptimizationUnitValidationError> {
    let ValueDefinitionSite::Node { block, node } = definition.site else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    let literal = function
        .block(block)
        .and_then(|block| {
            usize::try_from(node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
        .ok_or(OptimizationUnitValidationError::CandidateOperandFactMismatch)?;
    let O::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = literal.operation
    else {
        return Err(OptimizationUnitValidationError::CandidateOperandFactMismatch);
    };
    Ok(MaterializedIntegerConstant {
        operation: psi_operation,
        result,
        scalar_type,
        value,
    })
}
