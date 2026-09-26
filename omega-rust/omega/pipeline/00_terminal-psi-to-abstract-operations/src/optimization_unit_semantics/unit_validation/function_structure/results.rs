//! Function-result signature validation across every return operation.
use crate::optimization_unit::PsiOptimizationFunction;
use crate::optimization_unit_semantics::OptimizationUnitValidationError;

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
                crate::abstract_operations::AbstractOperation::Return {
                    result,
                    scalar_type,
                    ..
                },
                crate::abstract_operations::AbstractFunctionResult::Scalar(signature),
            ) => *result == signature.value && *scalar_type == signature.scalar_type,
            (
                crate::abstract_operations::AbstractOperation::ReturnUnit { .. },
                crate::abstract_operations::AbstractFunctionResult::Unit,
            )
            | (
                crate::abstract_operations::AbstractOperation::ReturnStructural { .. },
                crate::abstract_operations::AbstractFunctionResult::Structural(_),
            ) => true,
            (
                crate::abstract_operations::AbstractOperation::Return { .. }
                | crate::abstract_operations::AbstractOperation::ReturnUnit { .. }
                | crate::abstract_operations::AbstractOperation::ReturnStructural { .. },
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
