//! Independent replay of constant integer ordering materialized as a Boolean immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedIntegerLessThanImmediate;
use super::{
    StraightLineIntegerLessThanImmediateTranslationError as Error,
    StraightLineIntegerLessThanImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedIntegerLessThanImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.less_than_operation,
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
            && source_value == source.less_than_result
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.less_than_operation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.less_than_result,
        source.scalar_type,
        source.left_value,
        source.right_value,
        source.materialized_value,
    ))
}
