//! Independent target replay for a proof-bearing exact cast over an immediate operand.

use target_operations::{TargetFunction, TargetIntegerExpression, TargetOperation};

use super::grammar::ReconstructedIntegerExactCastImmediateOperand;
use super::{
    StraightLineIntegerExactCastImmediateOperandTranslationError as Error,
    StraightLineIntegerExactCastImmediateOperandTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedIntegerExactCastImmediateOperand,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice() != [source.constant_operation, source.cast_operation]
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
            expression: TargetIntegerExpression::IntegerExactCast {
                psi_operation,
                obligation,
                source_type,
                operand,
            },
        } if *psi_edge == source.return_edge
            && *source_value == source.cast_result
            && *scalar_type == source.target_type
            && *psi_operation == source.cast_operation
            && *obligation == source.obligation
            && *source_type == source.source_type
            && matches!(
                operand.as_ref(),
                TargetIntegerExpression::Immediate { source_value, value }
                    if *source_value == source.constant_result && *value == source.source_value
            )
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.constant_operation,
        source.cast_operation,
        source.obligation,
        source.return_edge,
        source.constant_result,
        source.cast_result,
        source.source_type,
        source.target_type,
        source.source_value,
        source.cast_value,
    ))
}
