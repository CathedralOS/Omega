//! Independent replay of constant integer wrapping integer multiplication as an integer immediate.

use target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedWrappingIntegerMultiplyImmediate;
use super::{
    StraightLineWrappingIntegerMultiplyImmediateTranslationError as Error,
    StraightLineWrappingIntegerMultiplyImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedWrappingIntegerMultiplyImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.wrapping_multiply_operation,
        ]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(Error::TargetProvenance);
    }
    if !matches!(
        target.operation,
        TargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } if psi_edge == source.return_edge
            && source_value == source.wrapping_multiply_result
            && scalar_type == source.scalar_type
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.wrapping_multiply_operation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.wrapping_multiply_result,
        source.scalar_type,
        source.left_value,
        source.right_value,
        source.materialized_value,
    ))
}
