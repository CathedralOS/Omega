//! Independent target replay for a constant bitwise complement materialized as an immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedIntegerBitwiseNotImmediate;
use super::{
    StraightLineIntegerBitwiseNotImmediateTranslationError as Error,
    StraightLineIntegerBitwiseNotImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedIntegerBitwiseNotImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [source.constant_operation, source.bitwise_not_operation]
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
            && source_value == source.bitwise_not_result
            && scalar_type == source.scalar_type
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.constant_operation,
        source.bitwise_not_operation,
        source.return_edge,
        source.constant_result,
        source.bitwise_not_result,
        source.scalar_type,
        source.source_value,
        source.materialized_value,
    ))
}
