//! Evaluate already-selected scalar operations over caller-owned value facts.
//! This adapter supplies no source binding, initializer, call, or range premise.

use checked_trees::{
    CheckedBooleanExpression, CheckedIntegerBinaryKind, CheckedIntegerComparisonKind,
    CheckedScalarExpression,
};
use numerics::{bignum::BigInt, literals::LandedIntegerType};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use typed_trees::types::PrimitiveType;

use facts::ScalarValue;

mod sources;
pub(crate) use sources::{BoundScalarValues, ScalarValueSource};

pub(crate) fn evaluate(
    expression: &CheckedScalarExpression,
    resolve_binding: &mut impl ScalarValueSource,
) -> Option<ScalarValue> {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type: PrimitiveType::Bool,
        } => resolve_binding
            .storage(*symbol)
            .filter(|value| matches!(value, ScalarValue::Boolean(_))),
        CheckedScalarExpression::Boolean(expression) => {
            boolean(expression, resolve_binding).map(ScalarValue::Boolean)
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type: PrimitiveType::Bool,
        }
        | CheckedScalarExpression::Local {
            position,
            primitive_type: PrimitiveType::Bool,
        } => binding_boolean(*position, resolve_binding).map(ScalarValue::Boolean),
        _ => integer(expression, resolve_binding)
            .map(|(_, value)| ScalarValue::Integer(integer_magnitude(value))),
    }
}

fn integer(
    expression: &CheckedScalarExpression,
    resolve_binding: &mut impl ScalarValueSource,
) -> Option<(IntegerType, IntegerValue)> {
    match expression {
        CheckedScalarExpression::StorageRead {
            symbol,
            primitive_type,
        } => {
            let scalar_type = integer_type(*primitive_type)?;
            let ScalarValue::Integer(value) = resolve_binding.storage(*symbol)? else {
                return None;
            };
            Some((scalar_type, admitted_integer(scalar_type, &value)?))
        }
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        }
        | CheckedScalarExpression::Local {
            position,
            primitive_type,
        } => {
            let scalar_type = integer_type(*primitive_type)?;
            let ScalarValue::Integer(value) = resolve_binding.binding(*position)? else {
                return None;
            };
            Some((scalar_type, admitted_integer(scalar_type, &value)?))
        }
        CheckedScalarExpression::IntegerLiteral { literal } => {
            let landed = literal.landing()?.landed_type;
            // This target-neutral evaluator receives no address-width
            // authority. A fixed-width integer law cannot select one for it.
            if landed == LandedIntegerType::Addr {
                return None;
            }
            let scalar_type = IntegerType::new(
                if landed.is_signed() {
                    IntegerSign::Signed
                } else {
                    IntegerSign::Unsigned
                },
                u16::try_from(landed.bit_width()).ok()?,
            )
            .ok()?;
            Some((
                scalar_type,
                admitted_integer(scalar_type, &literal.value_bignum()?)?,
            ))
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => {
            let scalar_type = integer_type(*primitive_type)?;
            let (left_type, left) = integer(left, resolve_binding)?;
            let (right_type, right) = integer(right, resolve_binding)?;
            if left_type != scalar_type {
                return None;
            }
            let shift = matches!(
                kind,
                CheckedIntegerBinaryKind::WrappingShiftLeft
                    | CheckedIntegerBinaryKind::WrappingShiftRight
                    | CheckedIntegerBinaryKind::ExactShiftLeft
                    | CheckedIntegerBinaryKind::ExactShiftRight
            );
            if !shift && right_type != scalar_type {
                return None;
            }
            Some((
                scalar_type,
                binary(*kind, scalar_type, right_type, left, right)?,
            ))
        }
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            let scalar_type = integer_type(*primitive_type)?;
            let (operand_type, value) = integer(operand, resolve_binding)?;
            if operand_type != scalar_type {
                return None;
            }
            Some((scalar_type, scalar_type.bitwise_not(value)?))
        }
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => {
            let target = integer_type(*primitive_type)?;
            let (source, value) = integer(operand, resolve_binding)?;
            Some((target, source.widen_value_to(target, value)?))
        }
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => {
            let target = integer_type(*primitive_type)?;
            let (source, value) = integer(operand, resolve_binding)?;
            let magnitude = integer_magnitude(value);
            if magnitude < range.minimum || magnitude > range.maximum {
                return None;
            }
            Some((target, source.exact_cast_value_to(target, value)?))
        }
        CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => None,
    }
}

fn binary(
    kind: CheckedIntegerBinaryKind,
    scalar_type: IntegerType,
    count_type: IntegerType,
    left: IntegerValue,
    right: IntegerValue,
) -> Option<IntegerValue> {
    match kind {
        CheckedIntegerBinaryKind::ExactAdd => scalar_type.exact_add(left, right),
        CheckedIntegerBinaryKind::ExactSubtract => scalar_type.exact_sub(left, right),
        CheckedIntegerBinaryKind::ExactMultiply => scalar_type.exact_mul(left, right),
        CheckedIntegerBinaryKind::ExactDivide => scalar_type.exact_div(left, right),
        CheckedIntegerBinaryKind::ExactRemainder => scalar_type.exact_rem(left, right),
        CheckedIntegerBinaryKind::WrappingAdd => scalar_type.wrapping_add(left, right),
        CheckedIntegerBinaryKind::WrappingSubtract => scalar_type.wrapping_sub(left, right),
        CheckedIntegerBinaryKind::WrappingMultiply => scalar_type.wrapping_mul(left, right),
        CheckedIntegerBinaryKind::WrappingDivide => scalar_type.wrapping_div(left, right),
        CheckedIntegerBinaryKind::WrappingRemainder => scalar_type.wrapping_rem(left, right),
        CheckedIntegerBinaryKind::SaturatingAdd => scalar_type.saturating_add(left, right),
        CheckedIntegerBinaryKind::SaturatingSubtract => scalar_type.saturating_sub(left, right),
        CheckedIntegerBinaryKind::SaturatingMultiply => scalar_type.saturating_mul(left, right),
        CheckedIntegerBinaryKind::SaturatingDivide => scalar_type.saturating_div(left, right),
        CheckedIntegerBinaryKind::SaturatingRemainder => scalar_type.saturating_rem(left, right),
        CheckedIntegerBinaryKind::BitwiseAnd => scalar_type.bitwise_and(left, right),
        CheckedIntegerBinaryKind::BitwiseOr => scalar_type.bitwise_or(left, right),
        CheckedIntegerBinaryKind::BitwiseXor => scalar_type.bitwise_xor(left, right),
        CheckedIntegerBinaryKind::WrappingShiftLeft => {
            scalar_type.wrapping_shift_left(left, count_type, right)
        }
        CheckedIntegerBinaryKind::WrappingShiftRight => {
            scalar_type.wrapping_shift_right(left, count_type, right)
        }
        CheckedIntegerBinaryKind::ExactShiftLeft => {
            scalar_type.exact_shift_left(left, count_type, right)
        }
        CheckedIntegerBinaryKind::ExactShiftRight => {
            scalar_type.exact_shift_right(left, count_type, right)
        }
    }
}

fn boolean(
    expression: &CheckedBooleanExpression,
    resolve_binding: &mut impl ScalarValueSource,
) -> Option<bool> {
    match expression {
        CheckedBooleanExpression::StorageRead { symbol } => {
            match resolve_binding.storage(*symbol)? {
                ScalarValue::Boolean(value) => Some(value),
                _ => None,
            }
        }
        CheckedBooleanExpression::Constant(value) => Some(*value),
        CheckedBooleanExpression::Parameter { position }
        | CheckedBooleanExpression::Local { position } => {
            binding_boolean(*position, resolve_binding)
        }
        CheckedBooleanExpression::Not(operand) => Some(!boolean(operand, resolve_binding)?),
        CheckedBooleanExpression::Equal { left, right } => {
            Some(boolean(left, resolve_binding)? == boolean(right, resolve_binding)?)
        }
        CheckedBooleanExpression::And { left, right } => {
            if boolean(left, resolve_binding)? {
                boolean(right, resolve_binding)
            } else {
                Some(false)
            }
        }
        CheckedBooleanExpression::Or { left, right } => {
            if boolean(left, resolve_binding)? {
                Some(true)
            } else {
                boolean(right, resolve_binding)
            }
        }
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let (left_type, left) = integer(left, resolve_binding)?;
            let (right_type, right) = integer(right, resolve_binding)?;
            if left_type != right_type {
                return None;
            }
            let ordering = left_type.compare(left, right)?;
            Some(match kind {
                CheckedIntegerComparisonKind::Equal => ordering.is_eq(),
                CheckedIntegerComparisonKind::LessThan => ordering.is_lt(),
                CheckedIntegerComparisonKind::LessOrEqual => !ordering.is_gt(),
            })
        }
        CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => None,
    }
}

fn binding_boolean(position: usize, resolve_binding: &mut impl ScalarValueSource) -> Option<bool> {
    match resolve_binding.binding(position)? {
        ScalarValue::Boolean(value) => Some(value),
        ScalarValue::Integer(_) | ScalarValue::Unknown => None,
    }
}

fn integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Addr | PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 => {
            return None;
        }
    };
    IntegerType::new(sign, bits).ok()
}

fn admitted_integer(scalar_type: IntegerType, value: &BigInt) -> Option<IntegerValue> {
    let value = match scalar_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(value.to_i64()?)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value.to_u64()?)),
    };
    scalar_type.admits(value).then_some(value)
}

fn integer_magnitude(value: IntegerValue) -> BigInt {
    match value {
        IntegerValue::Signed(value) => BigInt::from_i128(value),
        IntegerValue::Unsigned(value) => BigInt::from_u128(value),
    }
}

#[cfg(test)]
mod tests;
