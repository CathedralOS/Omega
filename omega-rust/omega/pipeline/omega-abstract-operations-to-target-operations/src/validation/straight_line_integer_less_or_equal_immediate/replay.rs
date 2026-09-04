//! Independent replay of constant inclusive integer ordering as a Boolean immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedIntegerLessOrEqualImmediate;
use super::{
    StraightLineIntegerLessOrEqualImmediateTranslationError as Error,
    StraightLineIntegerLessOrEqualImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedIntegerLessOrEqualImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.less_or_equal_operation,
        ]
        || target.provenance.edges.as_slice() != [source.return_edge]
    {
        return Err(Error::TargetProvenance);
    }
    if !matches!(
        target.operation,
        TargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        } if psi_edge == source.return_edge
            && source_value == source.less_or_equal_result
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.less_or_equal_operation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.less_or_equal_result,
        source.scalar_type,
        source.left_value,
        source.right_value,
        source.materialized_value,
    ))
}
