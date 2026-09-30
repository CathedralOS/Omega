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
    leaf: &impl Fn(&CheckedScalarExpression) -> Result<ScalarTerm, LoweringError>,
) -> Result<ScalarTerm, LoweringError> {
    let lower_operand = |operand| lower(operand, runtime_requirements, leaf);
    match expression {
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => {
            let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)? else {
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
        } if matches!(
            kind,
            CheckedIntegerBinaryKind::BitwiseAnd
                | CheckedIntegerBinaryKind::BitwiseOr
                | CheckedIntegerBinaryKind::BitwiseXor
        ) =>
        {
            let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)? else {
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
        } if matches!(
            kind,
            CheckedIntegerBinaryKind::WrappingShiftLeft
                | CheckedIntegerBinaryKind::WrappingShiftRight
                | CheckedIntegerBinaryKind::ExactShiftLeft
                | CheckedIntegerBinaryKind::ExactShiftRight
        ) =>
        {
            let ScalarType::Integer(value_type) = integer_scalar_type(*primitive_type)? else {
                return unsupported("structural crash shift has a non-integer value type");
            };
            let value = lower_operand(left)?;
            let count = lower_operand(right)?;
            if value.scalar_type() != ScalarType::Integer(value_type) {
                return unsupported("structural crash shift value does not match its integer type");
            }
            let ScalarType::Integer(count_type) = count.scalar_type() else {
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
            let ScalarType::Integer(integer_type) = integer_scalar_type(*primitive_type)? else {
                return unsupported("structural crash arithmetic has a non-integer type");
            };
            let left = Box::new(lower_operand(left)?);
            let right = Box::new(lower_operand(right)?);
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
            Ok(match kind {
                CheckedIntegerBinaryKind::ExactAdd => ScalarTerm::ExactIntegerAdd {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::ExactSubtract => ScalarTerm::ExactIntegerSubtract {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::ExactMultiply => ScalarTerm::ExactIntegerMultiply {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::ExactDivide => ScalarTerm::ExactIntegerDivide {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::ExactRemainder => ScalarTerm::ExactIntegerRemainder {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::WrappingAdd => ScalarTerm::WrappingIntegerAdd {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::SaturatingAdd => ScalarTerm::SaturatingIntegerAdd {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::WrappingSubtract => ScalarTerm::WrappingIntegerSubtract {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::SaturatingSubtract => {
                    ScalarTerm::SaturatingIntegerSubtract {
                        scalar_type: integer_type,
                        left,
                        right,
                    }
                }
                CheckedIntegerBinaryKind::WrappingMultiply => ScalarTerm::WrappingIntegerMultiply {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::SaturatingMultiply => {
                    ScalarTerm::SaturatingIntegerMultiply {
                        scalar_type: integer_type,
                        left,
                        right,
                    }
                }
                CheckedIntegerBinaryKind::WrappingDivide => ScalarTerm::WrappingIntegerDivide {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::WrappingRemainder => {
                    ScalarTerm::WrappingIntegerRemainder {
                        scalar_type: integer_type,
                        left,
                        right,
                    }
                }
                CheckedIntegerBinaryKind::SaturatingDivide => ScalarTerm::SaturatingIntegerDivide {
                    scalar_type: integer_type,
                    left,
                    right,
                },
                CheckedIntegerBinaryKind::SaturatingRemainder => {
                    ScalarTerm::SaturatingIntegerRemainder {
                        scalar_type: integer_type,
                        left,
                        right,
                    }
                }
                _ => unreachable!("guarded structural arithmetic kind"),
            })
        }
        _ => leaf(expression),
    }
}
