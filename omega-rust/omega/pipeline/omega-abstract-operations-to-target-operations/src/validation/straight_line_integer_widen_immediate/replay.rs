//! Independent target replay for constant widening materialized as an immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedIntegerWidenImmediate;
use super::{
    StraightLineIntegerWidenImmediateTranslationError as Error,
    StraightLineIntegerWidenImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedIntegerWidenImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [source.constant_operation, source.widen_operation]
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
            && source_value == source.widened_result
            && scalar_type == source.target_type
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.constant_operation,
        source.widen_operation,
        source.return_edge,
        source.constant_result,
        source.widened_result,
        source.source_type,
        source.target_type,
        source.source_value,
        source.materialized_value,
    ))
}
