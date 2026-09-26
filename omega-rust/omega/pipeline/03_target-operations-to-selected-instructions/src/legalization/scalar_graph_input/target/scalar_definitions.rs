//! Join ordered scalar definitions to their exact prior SSA sources.
use super::super::{ScalarType, ValueShape, scalar_shape};
use super::{AbstractOperation, TargetScalarExpression, TargetUnitOperation, ValueId};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::target::Checker;
use crate::legalization::scalar_graph_input::target::Expression;
use crate::legalization::scalar_graph_input::target::location_matches;
use crate::legalization::scalar_graph_input::{
    saturating_carrier, supports_wrapping_division, trapping_form,
};
use target_operations::{ScalarAbiValue, TargetUnitScalarArgumentSource as Source};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    scalar_parameters: &[ScalarAbiValue],
    sources: &[(ValueId, Source)],
) -> Result<(), LegalizationError> {
    let (
        TargetUnitOperation::ScalarDefinition {
            result_home,
            expression:
                TargetScalarExpression::Integer {
                    scalar_type,
                    expression:
                        Expression::IntegerWiden {
                            psi_operation,
                            source_type,
                            operand,
                        },
                },
        },
        AbstractOperation::IntegerWiden {
            psi_operation: expected_operation,
            result,
            source_type: expected_source_type,
            target_type,
            operand: source_value,
        },
    ) = (target, abstracted)
    else {
        return Err(LegalizationError::custody());
    };
    if psi_operation != expected_operation
        || source_type != expected_source_type
        || scalar_type != target_type
        || !source_type.can_widen_to(*target_type)
        || !matches!(target_type.bits(), 16 | 32 | 64)
        || result_home.defining_operation != *expected_operation
        || result_home.source_value != *result
        || result_home.scalar_type != ScalarType::Integer(*target_type)
        || result_home.shape != ValueShape::integer(target_type.bits() / 8, target_type.bits() / 8)
    {
        return Err(LegalizationError::custody());
    }
    let mut definitions = sources.iter().filter(|(value, _)| value == source_value);
    let definition = &definitions.next().ok_or(LegalizationError::custody())?.1;
    if definitions.next().is_some() {
        return Err(LegalizationError::custody());
    }
    let matches = match (operand.as_ref(), definition) {
        (
            Expression::Parameter {
                source_value: value,
                parameter_index,
                location,
            },
            Source::Parameter {
                source_value: expected_value,
                parameter_index: expected_index,
                scalar_type,
            },
        ) => {
            value == source_value
                && value == expected_value
                && usize::try_from(*expected_index).ok() == Some(*parameter_index)
                && *scalar_type == ScalarType::Integer(*source_type)
                && scalar_parameters
                    .get(*parameter_index)
                    .is_some_and(|parameter| {
                        parameter.value == *value
                            && parameter.scalar_type == *scalar_type
                            && location_matches(*location, &parameter.placement)
                    })
        }
        (
            Expression::Immediate {
                source_value: value,
                value: literal,
            },
            Source::IntegerImmediate {
                source_value: expected_value,
                scalar_type,
                value: expected_literal,
                ..
            },
        ) => {
            value == source_value
                && value == expected_value
                && scalar_type == source_type
                && literal == expected_literal
                && source_type.admits(*literal)
        }
        (Expression::BlockParameter(parameter), Source::BlockParameter(expected)) => {
            parameter == expected
                && parameter.value == *source_value
                && parameter.scalar_type == ScalarType::Integer(*source_type)
        }
        (Expression::ScalarHome(home), Source::Home(expected)) => {
            home == expected
                && home.source_value == *source_value
                && home.scalar_type == ScalarType::Integer(*source_type)
        }
        _ => false,
    };
    if !matches {
        return Err(LegalizationError::custody());
    }
    Ok(())
}

/// Scalar definitions retain ordered result homes; operands use prior homes.
/// The scalar families a Unit body's `ScalarDefinition` replays through
/// `observation`. The one list both the Unit operation replay and this
/// module read, so a family the scalar graph legalizes cannot be admitted
/// here and missed there.
pub(super) fn observed_family(abstracted: &AbstractOperation) -> bool {
    matches!(
        abstracted,
        AbstractOperation::ByteSequenceLength { .. }
            | AbstractOperation::ByteSequenceRead { .. }
            | AbstractOperation::ElementViewLength { .. }
            | AbstractOperation::ElementViewRead { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::BooleanEqual { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerDivide { .. }
            | AbstractOperation::SaturatingIntegerRemainder { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::TrappingInteger { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::WrappingIntegerDivide { .. }
            | AbstractOperation::WrappingIntegerRemainder { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerExactCast { .. }
    )
}

pub(super) fn observation(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    checker: &Checker<'_>,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::ScalarDefinition {
        result_home,
        expression,
    } = target
    else {
        return Err(LegalizationError::custody());
    };
    let (operation, value, scalar_type) = match abstracted {
        AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            if !saturating_carrier(*scalar_type).is_some()
                || checker.available.is_none_or(|sources| {
                    [left, right].iter().any(|operand| {
                        !sources.iter().any(|(value, source)| {
                            value == *operand
                                && source.scalar_type() == ScalarType::Integer(*scalar_type)
                        })
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            if !supports_wrapping_division(*scalar_type)
                || checker.available.is_none_or(|sources| {
                    [left, right].iter().any(|operand| {
                        !sources.iter().any(|(value, source)| {
                            value == *operand
                                && source.scalar_type() == ScalarType::Integer(*scalar_type)
                        })
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        // Every Trapping operand resolves through an available source of its
        // own declared type: the result carrier for binary operands and a
        // shifted value, the operand type for a count or conversion source.
        AbstractOperation::TrappingInteger {
            psi_operation,
            result,
            scalar_type,
            operand_type,
            operation,
        } => {
            use terminal_psi::TrappingIntegerOperation as T;
            let typed = |operand: ValueId, expected| {
                checker.available.is_some_and(|sources| {
                    sources.iter().any(|(value, source)| {
                        *value == operand && source.scalar_type() == ScalarType::Integer(expected)
                    })
                })
            };
            let operands_available = match *operation {
                T::Add { left, right }
                | T::Subtract { left, right }
                | T::Multiply { left, right }
                | T::Divide { left, right }
                | T::Remainder { left, right } => {
                    typed(left, *scalar_type) && typed(right, *scalar_type)
                }
                T::ShiftLeft { value, count } | T::ShiftRight { value, count } => {
                    typed(value, *scalar_type) && typed(count, *operand_type)
                }
                T::Convert { operand } => typed(operand, *operand_type),
            };
            if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_none()
                || !operands_available
            {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            source_type,
            target_type,
            operand,
            ..
        } => {
            if !source_type.can_exact_cast_to(*target_type)
                || checker.available.is_none_or(|sources| {
                    !sources.iter().any(|(value, source)| {
                        value == operand
                            && source.scalar_type() == ScalarType::Integer(*source_type)
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*target_type))
        }
        AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            operand,
        } => {
            if checker.available.is_none_or(|sources| {
                !sources.iter().any(|(value, source)| {
                    value == operand && source.scalar_type() == ScalarType::Integer(*scalar_type)
                })
            }) {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::SaturatingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::WrappingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::WrappingIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::WrappingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::SaturatingIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::SaturatingIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerMultiply {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerDivide {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        }
        | AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
            ..
        } => {
            if checker.available.is_none_or(|sources| {
                [left, right].iter().any(|operand| {
                    !sources.iter().any(|(value, source)| {
                        value == *operand
                            && source.scalar_type() == ScalarType::Integer(*scalar_type)
                    })
                })
            }) {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
        }
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        }
        | AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
            ..
        } => {
            // The count is independently typed: each operand must resolve
            // through an available source carrying its own declared type.
            if scalar_shape(ScalarType::Integer(*value_type)).is_none()
                || scalar_shape(ScalarType::Integer(*count_type)).is_none()
                || checker.available.is_none_or(|sources| {
                    !sources.iter().any(|(identity, source)| {
                        identity == value
                            && source.scalar_type() == ScalarType::Integer(*value_type)
                    }) || !sources.iter().any(|(identity, source)| {
                        identity == count
                            && source.scalar_type() == ScalarType::Integer(*count_type)
                    })
                })
            {
                return Err(LegalizationError::custody());
            }
            (*psi_operation, *result, ScalarType::Integer(*value_type))
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            ..
        } => (*psi_operation, result.value, result.scalar_type),
        AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanNot {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::BooleanEqual {
            psi_operation,
            result,
            ..
        } => (*psi_operation, *result, ScalarType::Boolean),
        _ => return Err(LegalizationError::custody()),
    };
    if result_home.defining_operation != operation
        || result_home.source_value != value
        || result_home.scalar_type != scalar_type
        || Some(result_home.shape) != scalar_shape(scalar_type)
    {
        return Err(LegalizationError::custody());
    }
    let matches = match (expression, scalar_type) {
        (
            TargetScalarExpression::Integer {
                scalar_type: actual,
                expression,
            },
            ScalarType::Integer(expected),
        ) => *actual == expected && checker.expression(expression, value, &[]),
        (TargetScalarExpression::Boolean(expression), ScalarType::Boolean) => {
            checker.boolean(expression, value, &[])
        }
        _ => false,
    };
    if !matches {
        return Err(LegalizationError::custody());
    }
    Ok(())
}
