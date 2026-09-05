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
                abstract_operations::AbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                abstract_operations::AbstractFunctionResult::Scalar(signature),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                abstract_operations::AbstractOperation::ReturnUnit { .. },
                abstract_operations::AbstractFunctionResult::Unit,
            )
            | (
                abstract_operations::AbstractOperation::ReturnStructural { .. },
                abstract_operations::AbstractFunctionResult::Structural(_),
            ) => true,
            (
                abstract_operations::AbstractOperation::Return { .. }
                | abstract_operations::AbstractOperation::ReturnUnit { .. }
                | abstract_operations::AbstractOperation::ReturnStructural { .. },
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
