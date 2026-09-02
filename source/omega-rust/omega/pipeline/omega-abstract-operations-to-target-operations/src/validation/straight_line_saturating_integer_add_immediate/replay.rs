//! Independent replay of constant saturating integer addition as an immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedSaturatingIntegerAddImmediate;
use super::{
    StraightLineSaturatingIntegerAddImmediateTranslationError as Error,
    StraightLineSaturatingIntegerAddImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedSaturatingIntegerAddImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.saturating_add_operation,
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
            && source_value == source.saturating_add_result
            && scalar_type == source.scalar_type
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.saturating_add_operation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.saturating_add_result,
        source.scalar_type,
        source.left_value,
        source.right_value,
        source.materialized_value,
    ))
}
