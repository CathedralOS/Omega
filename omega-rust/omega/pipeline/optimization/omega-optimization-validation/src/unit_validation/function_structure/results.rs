//! Function-result signature validation across every return operation.

use super::*;

pub(super) fn validate_function_results(
    function: &PsiOptimizationFunction,
) -> Result<(), OptimizationUnitValidationError> {
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
    {
        let matches = match (operation, &function.result) {
            (
                omega_abstract_operations::AbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                omega_abstract_operations::AbstractFunctionResult::Scalar(signature),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                omega_abstract_operations::AbstractOperation::ReturnUnit { .. },
                omega_abstract_operations::AbstractFunctionResult::Unit,
            )
            | (
                omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                omega_abstract_operations::AbstractFunctionResult::Structural(_),
            ) => true,
            (
                omega_abstract_operations::AbstractOperation::Return { .. }
                | omega_abstract_operations::AbstractOperation::ReturnUnit { .. }
                | omega_abstract_operations::AbstractOperation::ReturnStructural { .. },
                _,
            ) => false,
            _ => continue,
        };
        if !matches {
            return Err(OptimizationUnitValidationError::FunctionResultMismatch(
                function.machine,
            ));
        }
    }
    Ok(())
}
