//! Independent replay of proof-bearing wrapping divide over constant operands.

use target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::grammar::ReconstructedWrappingIntegerDivideImmediateOperands;
use super::{
    StraightLineWrappingIntegerDivideImmediateOperandsTranslationError as Error,
    StraightLineWrappingIntegerDivideImmediateOperandsTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedWrappingIntegerDivideImmediateOperands,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.divide_operation,
        ]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(Error::TargetProvenance);
    }
    if !matches!(
        &target.operation,
        TargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression: TargetIntegerExpression::WrappingDivide {
                psi_operation,
                obligation,
                left,
                right,
            },
        } if *psi_edge == source.return_edge
            && *source_value == source.divide_result
            && *scalar_type == source.scalar_type
            && *psi_operation == source.divide_operation
            && *obligation == source.obligation
            && matches!(
                left.as_ref(),
                TargetIntegerExpression::Immediate { source_value, value }
                    if *source_value == source.left_constant_result && *value == source.left
            )
            && matches!(
                right.as_ref(),
                TargetIntegerExpression::Immediate { source_value, value }
                    if *source_value == source.right_constant_result && *value == source.right
            )
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.divide_operation,
        source.obligation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.divide_result,
        source.scalar_type,
        source.left,
        source.right,
        source.quotient,
    ))
}
