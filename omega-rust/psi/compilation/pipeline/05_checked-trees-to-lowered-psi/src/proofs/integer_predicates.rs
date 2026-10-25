//! Integer predicate operations shared by entry requirements and crash routes.
//! Callers own leaf namespaces; this conversion preserves carrier and policy
//! meaning. Partial operators still need explicit safety evidence, never the
//! requirement being translated as its own premise.

use super::crash_routes::structural_arithmetic::{
    safe_exact_structural_divisor, safe_exact_structural_shift, safe_policy_structural_divisor,
};
use super::{
    CheckedIntegerBinaryKind, CheckedScalarExpression, LoweringError, Proposition, ScalarTerm,
    ScalarType, integer_scalar_type, unsupported,
};

pub(crate) fn lower(
    expression: &CheckedScalarExpression,
    runtime_requirements: &[Proposition],
    leaf: &mut impl FnMut(&CheckedScalarExpression) -> Result<ScalarTerm, LoweringError>,
) -> Result<ScalarTerm, LoweringError> {
    let mut lower_operand = |operand| lower(operand, runtime_requirements, leaf);
    match expression {
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                return unsupported("structural crash bitwise-not has a non-integer type");
            };
            let operand = lower_operand(operand)?;
            ScalarTerm::integer_bitwise_not(integer_type, operand)
                .map_err(LoweringError::InvalidCrashPredicate)
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } if matches!(
            kind,
            CheckedIntegerBinaryKind::BitwiseAnd
                | CheckedIntegerBinaryKind::BitwiseOr
                | CheckedIntegerBinaryKind::BitwiseXor
        ) =>
        {
            let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                return unsupported("structural crash bitwise expression has a non-integer type");
            };
            let left = lower_operand(left)?;
            let right = lower_operand(right)?;
            match kind {
                CheckedIntegerBinaryKind::BitwiseAnd => {
                    ScalarTerm::integer_bitwise_and(integer_type, left, right)
                }
                CheckedIntegerBinaryKind::BitwiseOr => {
                    ScalarTerm::integer_bitwise_or(integer_type, left, right)
                }
                CheckedIntegerBinaryKind::BitwiseXor => {
                    ScalarTerm::integer_bitwise_xor(integer_type, left, right)
                }
                _ => unreachable!("guarded bitwise kind"),
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } if matches!(
            kind,
            CheckedIntegerBinaryKind::WrappingShiftLeft
                | CheckedIntegerBinaryKind::WrappingShiftRight
                | CheckedIntegerBinaryKind::ExactShiftLeft
                | CheckedIntegerBinaryKind::ExactShiftRight
        ) =>
        {
            let Some(value_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                return unsupported("structural crash shift has a non-integer value type");
            };
            let value = lower_operand(left)?;
            let count = lower_operand(right)?;
            if value.scalar_type() != ScalarType::Integer(value_type) {
                return unsupported("structural crash shift value does not match its integer type");
            }
            let Some(count_type) = count.scalar_type().as_integer() else {
                return unsupported("structural crash shift count is not an integer");
            };
            if matches!(
                kind,
                CheckedIntegerBinaryKind::ExactShiftLeft
                    | CheckedIntegerBinaryKind::ExactShiftRight
            ) && !safe_exact_structural_shift(
                matches!(kind, CheckedIntegerBinaryKind::ExactShiftLeft),
                value_type,
                count_type,
                &value,
                &count,
                runtime_requirements,
            ) {
                return unsupported(
                    "structural crash Exact shift requires explicit terminal count and overflow safety evidence",
                );
            }
            match kind {
                CheckedIntegerBinaryKind::WrappingShiftLeft => {
                    ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                }
                CheckedIntegerBinaryKind::WrappingShiftRight => {
                    ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                }
                CheckedIntegerBinaryKind::ExactShiftLeft => {
                    ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                }
                CheckedIntegerBinaryKind::ExactShiftRight => {
                    ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                }
                _ => unreachable!("guarded structural shift kind"),
            }
            .map_err(LoweringError::InvalidCrashPredicate)
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
            ..
        } if matches!(
            kind,
            CheckedIntegerBinaryKind::ExactAdd
                | CheckedIntegerBinaryKind::ExactSubtract
                | CheckedIntegerBinaryKind::ExactMultiply
                | CheckedIntegerBinaryKind::ExactDivide
                | CheckedIntegerBinaryKind::ExactRemainder
                | CheckedIntegerBinaryKind::WrappingAdd
                | CheckedIntegerBinaryKind::SaturatingAdd
                | CheckedIntegerBinaryKind::WrappingSubtract
                | CheckedIntegerBinaryKind::SaturatingSubtract
                | CheckedIntegerBinaryKind::WrappingMultiply
                | CheckedIntegerBinaryKind::SaturatingMultiply
                | CheckedIntegerBinaryKind::WrappingDivide
                | CheckedIntegerBinaryKind::WrappingRemainder
                | CheckedIntegerBinaryKind::SaturatingDivide
                | CheckedIntegerBinaryKind::SaturatingRemainder
        ) =>
        {
            let Some(integer_type) = integer_scalar_type(*primitive_type)?.as_integer() else {
                return unsupported("structural crash arithmetic has a non-integer type");
            };
            let left = lower_operand(left)?;
            let right = lower_operand(right)?;
            if left.scalar_type() != ScalarType::Integer(integer_type)
                || right.scalar_type() != ScalarType::Integer(integer_type)
            {
                return unsupported(
                    "structural crash arithmetic operands do not match its integer type",
                );
            }
            if matches!(
                kind,
                CheckedIntegerBinaryKind::ExactDivide | CheckedIntegerBinaryKind::ExactRemainder
            ) && !safe_exact_structural_divisor(
                integer_type,
                &left,
                &right,
                runtime_requirements,
            ) {
                return unsupported(
                    "structural crash exact division requires explicit terminal divisor safety evidence",
                );
            }
            if matches!(
                kind,
                CheckedIntegerBinaryKind::WrappingDivide
                    | CheckedIntegerBinaryKind::WrappingRemainder
                    | CheckedIntegerBinaryKind::SaturatingDivide
                    | CheckedIntegerBinaryKind::SaturatingRemainder
            ) && !safe_policy_structural_divisor(integer_type, &right, runtime_requirements)
            {
                return unsupported(
                    "structural crash policy division requires explicit terminal nonzero-divisor evidence",
                );
            }
            // One construction row per guarded kind: each arm names the
            // ScalarTerm constructor that owns its operation identity, and the
            // operands apply once below.
            let constructor: fn(
                semantic_vocabulary::IntegerType,
                ScalarTerm,
                ScalarTerm,
            )
                -> Result<ScalarTerm, semantic_vocabulary::PropositionError> = match kind {
                CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::exact_integer_add,
                CheckedIntegerBinaryKind::ExactSubtract => ScalarTerm::exact_integer_subtract,
                CheckedIntegerBinaryKind::ExactMultiply => ScalarTerm::exact_integer_multiply,
                CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::exact_integer_divide,
                CheckedIntegerBinaryKind::ExactRemainder => ScalarTerm::exact_integer_remainder,
                CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::wrapping_integer_add,
                CheckedIntegerBinaryKind::SaturatingAdd => ScalarTerm::saturating_integer_add,
                CheckedIntegerBinaryKind::WrappingSubtract => ScalarTerm::wrapping_integer_subtract,
                CheckedIntegerBinaryKind::SaturatingSubtract => {
                    ScalarTerm::saturating_integer_subtract
                }
                CheckedIntegerBinaryKind::WrappingMultiply => ScalarTerm::wrapping_integer_multiply,
                CheckedIntegerBinaryKind::SaturatingMultiply => {
                    ScalarTerm::saturating_integer_multiply
                }
                CheckedIntegerBinaryKind::WrappingDivide => ScalarTerm::wrapping_integer_divide,
                CheckedIntegerBinaryKind::WrappingRemainder => {
                    ScalarTerm::wrapping_integer_remainder
                }
                CheckedIntegerBinaryKind::SaturatingDivide => ScalarTerm::saturating_integer_divide,
                CheckedIntegerBinaryKind::SaturatingRemainder => {
                    ScalarTerm::saturating_integer_remainder
                }
                _ => unreachable!("guarded structural arithmetic kind"),
            };
            Ok(constructor(integer_type, left, right)
                .map_err(LoweringError::InvalidCrashPredicate)?)
        }
        _ => leaf(expression),
    }
}
