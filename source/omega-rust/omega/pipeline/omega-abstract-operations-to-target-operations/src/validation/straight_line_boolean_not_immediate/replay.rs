//! Independent target replay for a constant Boolean negation materialized as an immediate.

use omega_target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedBooleanNotImmediate;
use super::{
    StraightLineBooleanNotImmediateTranslationError as Error,
    StraightLineBooleanNotImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedBooleanNotImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [source.constant_operation, source.boolean_not_operation]
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
            && source_value == source.boolean_not_result
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.constant_operation,
        source.boolean_not_operation,
        source.return_edge,
        source.constant_result,
        source.boolean_not_result,
        source.source_value,
        source.materialized_value,
    ))
}
