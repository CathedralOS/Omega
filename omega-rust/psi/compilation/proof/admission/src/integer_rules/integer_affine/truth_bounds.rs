//! The truth bounds an integer affine form admits, in i128 arithmetic.

use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
use crate::integer_rules::integer_affine::{
    CheckedIntegerAffineForm, IntegerAffineBoundConversionError,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm};

/// Derive the two carrier-tight endpoint relations for a checked word whose
/// landed nonzero exact divide or remainder has a total full-carrier image,
/// making its output range independent of the incoming root value. The caller
/// still supplies and recursively checks a `Truth` child so ordinary
/// proof-node custody remains explicit.
pub fn integer_affine_truth_bounds(
    form: &CheckedIntegerAffineForm,
) -> Result<Vec<Proposition>, IntegerAffineBoundConversionError> {
    let mut minimum = integer_value_to_i128(form.integer_type().minimum_value())
        .ok_or(IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?;
    let mut maximum = integer_value_to_i128(form.integer_type().maximum_value())
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
            // A wrapping step has no total endpoint image: its reduced result
            // is not a monotone function of a quantified root range.
            CheckedIntegerEndpointStep::WrappingAdd { .. }
            | CheckedIntegerEndpointStep::WrappingAddBackward { .. }
            | CheckedIntegerEndpointStep::CorrelatedAddLower
            | CheckedIntegerEndpointStep::CorrelatedAddUpper
            | CheckedIntegerEndpointStep::CorrelatedSubtractLower
            | CheckedIntegerEndpointStep::CorrelatedSubtractUpper
            | CheckedIntegerEndpointStep::CorrelatedUnsignedSubtract
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
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value),
        IntegerSign::Unsigned => IntegerValue::Unsigned(
            u128::try_from(value)
                .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)?,
        ),
    };
    ScalarTerm::integer(integer_type, value)
        .map_err(|_| IntegerAffineBoundConversionError::MappedBoundOutsideCarrier)
}

fn integer_value_to_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

pub(crate) fn integer_literal_as_i128(
    term: &ScalarTerm,
    integer_type: IntegerType,
) -> Option<i128> {
    let (actual_type, value) = term.integer_value()?;
    if actual_type != integer_type {
        return None;
    }
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}
