//! The truth bounds an integer affine form admits, in i128 arithmetic.

use crate::integer_rules::fixed_integer::integer_value_as_i128;
use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
use crate::integer_rules::integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm};

/// Derive the two endpoint relations for a checked word with a total image
/// over the root carrier: for example, a landed nonzero divide, remainder,
/// or Saturating translation. Saturation clamps the full carrier's endpoints,
/// so these inclusive bounds need no root premise. The caller still supplies
/// and recursively checks a `Truth` child to preserve proof-node custody.
pub fn integer_affine_truth_bounds(
    form: &CheckedIntegerAffineForm,
) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
    let mut minimum = integer_value_as_i128(form.integer_type().minimum_value())
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let mut maximum = integer_value_as_i128(form.integer_type().maximum_value())
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let carrier_minimum = minimum;
    let carrier_maximum = maximum;
    let mut saw_total_image = false;
    for step in &form.endpoint_steps {
        match step {
            CheckedIntegerEndpointStep::Add(value) => {
                minimum = minimum
                    .checked_add(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_add(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::SaturatingAdd(value) => {
                minimum = minimum
                    .checked_add(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    .clamp(carrier_minimum, carrier_maximum);
                maximum = maximum
                    .checked_add(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                    .clamp(carrier_minimum, carrier_maximum);
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::Subtract(value) => {
                minimum = minimum
                    .checked_sub(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_sub(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::Multiply(value) => {
                let left = minimum
                    .checked_mul(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                let right = maximum
                    .checked_mul(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                (minimum, maximum) = (left.min(right), left.max(right));
                if *value == 0 {
                    saw_total_image = true;
                }
            }
            CheckedIntegerEndpointStep::Divide(value) => {
                if form.integer_type().sign() == IntegerSign::Signed && *value == -1 {
                    return Err(IntegerAffineBoundConversionError::NonTotalDivisionImage);
                }
                let left = carrier_minimum
                    .checked_div(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                let right = carrier_maximum
                    .checked_div(*value)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                (minimum, maximum) = (left.min(right), left.max(right));
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::Remainder(value) => {
                let magnitude = value
                    .checked_abs()
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                minimum = if form.integer_type().sign() == IntegerSign::Signed {
                    1_i128
                        .checked_sub(magnitude)
                        .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?
                } else {
                    0
                };
                maximum = magnitude
                    .checked_sub(1)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                saw_total_image = true;
            }
            CheckedIntegerEndpointStep::ShiftLeft(count) => {
                if *count == 0 {
                    minimum = carrier_minimum;
                    maximum = carrier_maximum;
                    saw_total_image = true;
                    continue;
                }
                let scale = 1_i128
                    .checked_shl(*count)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                minimum = minimum
                    .checked_mul(scale)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
                maximum = maximum
                    .checked_mul(scale)
                    .ok_or(IntegerAffineBoundConversionError::MappedBoundOverflow)?;
            }
            CheckedIntegerEndpointStep::ShiftRight(count) => {
                minimum = carrier_minimum >> count;
                maximum = carrier_maximum >> count;
                saw_total_image = true;
            }
            // `x & mask` with a checked non-negative mask keeps only mask
            // bits: the image is `[0, mask]` on every fixed carrier and does
            // not depend on the root value.
            CheckedIntegerEndpointStep::BitwiseAndMask(mask) => {
                minimum = 0;
                maximum = *mask;
                saw_total_image = true;
            }
            // The bounded-mask reading keeps the same total image with the
            // axiom-proven upper bound in place of a literal mask.
            CheckedIntegerEndpointStep::BitwiseAndBoundedMask { upper, .. } => {
                minimum = 0;
                maximum = *upper;
                saw_total_image = true;
            }
            // A bitwise-or step has no total endpoint image either: `x | s`
            // over a quantified operand produces only values carrying the
            // sibling's set bits, which is not a contiguous interval.
            CheckedIntegerEndpointStep::BitwiseOrLiteralImage(_)
            | CheckedIntegerEndpointStep::BitwiseOrBoundedImage { .. } => {
                return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
            }
            // A wrapping step has no total endpoint image: its reduced result
            // is not a monotone function of a quantified root range.
            CheckedIntegerEndpointStep::WrappingAdd { .. }
            | CheckedIntegerEndpointStep::WrappingAddBackward { .. }
            | CheckedIntegerEndpointStep::CorrelatedAddLower
            | CheckedIntegerEndpointStep::CorrelatedAddUpper
            | CheckedIntegerEndpointStep::CorrelatedSubtractLower
            | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
            | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
            | CheckedIntegerEndpointStep::CorrelatedUnsignedRemainder
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMinimum
            | CheckedIntegerEndpointStep::CorrelatedMultiplyMaximum => {
                return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
            }
        }
    }
    if !saw_total_image {
        return Err(IntegerAffineBoundConversionError::TruthRootWithoutTotalImage);
    }
    let minimum = scalar_integer_from_i128(form.integer_type(), minimum)?;
    let maximum = scalar_integer_from_i128(form.integer_type(), maximum)?;
    Ok(vec![
        Proposition::LessOrEqual(minimum, form.target().clone()),
        Proposition::LessOrEqual(form.target().clone(), maximum),
    ])
}

fn scalar_integer_from_i128(
    integer_type: IntegerType,
    value: i128,
) -> Result<ScalarTerm, IntegerAffineBoundConversionError> {
    let value = IntegerValue::for_sign(integer_type.sign(), value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    ScalarTerm::integer(integer_type, value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)
}
