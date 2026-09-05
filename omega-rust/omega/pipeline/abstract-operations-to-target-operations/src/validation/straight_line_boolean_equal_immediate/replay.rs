//! Independent replay of constant Boolean equality materialized as an immediate.

use target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedBooleanEqualImmediate;
use super::{
    StraightLineBooleanEqualImmediateTranslationError as Error,
    StraightLineBooleanEqualImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedBooleanEqualImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.left_constant_operation,
            source.right_constant_operation,
            source.equal_operation,
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
            && source_value == source.equal_result
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.left_constant_operation,
        source.right_constant_operation,
        source.equal_operation,
        source.return_edge,
        source.left_constant_result,
        source.right_constant_result,
        source.equal_result,
        source.left_value,
        source.right_value,
        source.materialized_value,
    ))
}
