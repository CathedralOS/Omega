//! Independent replay of constant wrapping integer shift-left as an immediate.

use target_operations::{TargetFunction, TargetOperation};

use super::grammar::ReconstructedWrappingIntegerShiftLeftImmediate;
use super::{
    StraightLineWrappingIntegerShiftLeftImmediateTranslationError as Error,
    StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt as Receipt,
};

pub(super) fn validate(
    source: ReconstructedWrappingIntegerShiftLeftImmediate,
    target: &TargetFunction,
) -> Result<Receipt, Error> {
    if target.provenance.operations.as_slice()
        != [
            source.value_constant_operation,
            source.count_constant_operation,
            source.wrapping_shift_operation,
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
            && source_value == source.wrapping_shift_result
            && scalar_type == source.value_type
            && value == source.materialized_value
    ) {
        return Err(Error::TargetOperation);
    }
    Ok(Receipt::new(
        source.machine,
        source.value_constant_operation,
        source.count_constant_operation,
        source.wrapping_shift_operation,
        source.return_edge,
        source.value_constant_result,
        source.count_constant_result,
        source.wrapping_shift_result,
        source.value_type,
        source.count_type,
        source.value,
        source.count,
        source.materialized_value,
    ))
}
