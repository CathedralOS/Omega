//! Scalar operations of the interpreter loop and the scalar helpers the
//! rest of the runtime shares: type membership of a scalar value, IEEE-float
//! comparison and fused multiply-add.
//!
//! `integer.rs` executes the integer operations (constants, comparisons,
//! bitwise and width conversions, and the exact, wrapping and saturating
//! arithmetic families); `boolean_float.rs` the Boolean and IEEE-float
//! operations.

pub(crate) mod boolean_float;
pub(crate) mod integer;

use crate::values::TerminalScalarValue;
use numerics::float_semantics::{FloatFormat, FloatMeaning, FloatSemantics};
use semantic_vocabulary::{IeeeFloatFormat, IeeeFloatValue};

pub(crate) fn terminal_scalar_belongs_to_type(value: TerminalScalarValue) -> bool {
    match value {
        TerminalScalarValue::Boolean(_) => true,
        TerminalScalarValue::Integer { scalar_type, value } => scalar_type.admits(value),
        TerminalScalarValue::IeeeFloat(_) => true,
    }
}

/// Interpret representation bits through the common format semantics. Host
/// comparisons would be a second definition, particularly around NaNs.
pub(crate) fn ieee_float_compare(
    comparison: semantic_vocabulary::IeeeFloatComparisonOperation,
    left: IeeeFloatValue,
    right: IeeeFloatValue,
) -> bool {
    use semantic_vocabulary::IeeeFloatComparisonOperation;
    let meaning = |value| match value {
        IeeeFloatValue::Binary32(bits) => FloatMeaning::from_f32(f32::from_bits(bits)),
        IeeeFloatValue::Binary64(bits) => FloatMeaning::from_f64(f64::from_bits(bits)),
    };
    let left = meaning(left);
    let right = meaning(right);
    match comparison {
        IeeeFloatComparisonOperation::Equal => FloatSemantics::equal(&left, &right),
        IeeeFloatComparisonOperation::NotEqual => !FloatSemantics::equal(&left, &right),
        IeeeFloatComparisonOperation::Less => FloatSemantics::less(&left, &right),
        IeeeFloatComparisonOperation::LessOrEqual => FloatSemantics::less_or_equal(&left, &right),
        IeeeFloatComparisonOperation::Greater => FloatSemantics::greater(&left, &right),
        IeeeFloatComparisonOperation::GreaterOrEqual => {
            FloatSemantics::greater_or_equal(&left, &right)
        }
    }
}

pub(crate) fn nearest_ieee_float_fused_multiply_add(
    format: IeeeFloatFormat,
    left: IeeeFloatValue,
    right: IeeeFloatValue,
    addend: IeeeFloatValue,
) -> IeeeFloatValue {
    match (format, left, right, addend) {
        (
            IeeeFloatFormat::Binary32,
            IeeeFloatValue::Binary32(left),
            IeeeFloatValue::Binary32(right),
            IeeeFloatValue::Binary32(addend),
        ) => IeeeFloatValue::Binary32(
            FloatSemantics::fused_multiply_add(
                FloatFormat::BINARY32,
                &FloatMeaning::from_f32(f32::from_bits(left)),
                &FloatMeaning::from_f32(f32::from_bits(right)),
                &FloatMeaning::from_f32(f32::from_bits(addend)),
            )
            .to_f32()
            .to_bits(),
        ),
        (
            IeeeFloatFormat::Binary64,
            IeeeFloatValue::Binary64(left),
            IeeeFloatValue::Binary64(right),
            IeeeFloatValue::Binary64(addend),
        ) => IeeeFloatValue::Binary64(
            FloatSemantics::fused_multiply_add(
                FloatFormat::BINARY64,
                &FloatMeaning::from_f64(f64::from_bits(left)),
                &FloatMeaning::from_f64(f64::from_bits(right)),
                &FloatMeaning::from_f64(f64::from_bits(addend)),
            )
            .to_f64()
            .to_bits(),
        ),
        _ => unreachable!("verified IEEE FMA operands have one exact format"),
    }
}
