//! Authored floating range endpoints keep their exact IEEE interchange
//! values at the declared carrier. The source spelling stays authoritative
//! upstream; these readers convert once, directly from the spelling, and
//! never normalize an exclusive end through integer predecessor arithmetic.

use numerics::literals::FloatFormat;
use semantic_vocabulary::IeeeFloatValue;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::PrimitiveType;

/// Evaluate one authored floating range endpoint at its declared carrier.
/// A float literal reads its spelling directly at the carrier format; a
/// literal authored with a narrower landing keeps that value and widens
/// exactly, while a wider-authored literal cannot narrow through the f32
/// read. Integer endpoints keep their exact completed value converted once
/// into the carrier. Anything else cannot be retained, so range formation
/// (`constraint_validation`) refuses a floating range whose endpoint this
/// reader does not close.
pub fn closed_float_range_endpoint(
    program: &TypedTrees,
    expression: ExpressionHandle,
    primitive_type: PrimitiveType,
) -> Option<IeeeFloatValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Float(literal) => match primitive_type {
            PrimitiveType::F32 => (literal.landing() != Some(FloatFormat::F64))
                .then(|| IeeeFloatValue::Binary32(literal.value_f32().to_bits())),
            PrimitiveType::F64 => Some(IeeeFloatValue::Binary64(literal.landed_f64().to_bits())),
            _ => None,
        },
        // Mixed floating ranges may have integer endpoints; the completed
        // exact value converts once into the carrier, matching the proof
        // constraint read. A failed exact endpoint cannot fall back to its
        // raw literal text.
        ExpressionNode::Integer(_) | ExpressionNode::Name(_) => {
            let value =
                super::integer_ranges::closed_integer_range_bound(program, expression)?.to_i64()?;
            Some(match primitive_type {
                PrimitiveType::F32 => IeeeFloatValue::Binary32((value as f32).to_bits()),
                PrimitiveType::F64 => IeeeFloatValue::Binary64((value as f64).to_bits()),
                _ => return None,
            })
        }
        _ => None,
    }
}

/// IEEE `<=` over retained interchange values. NaN endpoints fail the
/// order, so an unordered or NaN-bounded window cannot be retained as a
/// nonempty requirement. Signed zeros order under IEEE equality.
pub fn ieee_float_range_ordered(minimum: IeeeFloatValue, maximum: IeeeFloatValue) -> bool {
    match (minimum, maximum) {
        (IeeeFloatValue::Binary32(low), IeeeFloatValue::Binary32(high)) => {
            f32::from_bits(low) <= f32::from_bits(high)
        }
        (IeeeFloatValue::Binary64(low), IeeeFloatValue::Binary64(high)) => {
            f64::from_bits(low) <= f64::from_bits(high)
        }
        _ => false,
    }
}
