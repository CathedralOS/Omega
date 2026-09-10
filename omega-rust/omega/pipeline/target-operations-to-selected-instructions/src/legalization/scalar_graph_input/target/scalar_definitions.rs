//! Join ordered scalar definitions to their exact prior SSA sources.
use super::*;
use target_operations::{ScalarAbiValue, TargetUnitScalarArgumentSource as Source};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    scalar_parameters: &[ScalarAbiValue],
    sources: &[(ValueId, Source)],
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
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
        return Err(invalid);
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
        return Err(invalid);
    }
    let mut definitions = sources.iter().filter(|(value, _)| value == source_value);
    let definition = &definitions.next().ok_or(invalid.clone())?.1;
    if definitions.next().is_some() {
        return Err(invalid);
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
        return Err(invalid);
    }
    Ok(())
}

/// Scalar definitions retain ordered result homes; operands use prior homes.
pub(super) fn observation(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    checker: &Checker<'_>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let TargetUnitOperation::ScalarDefinition {
        result_home,
        expression,
    } = target
    else {
        return Err(invalid);
    };
    let (operation, value, scalar_type) = match abstracted {
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
                return Err(invalid);
            }
            (*psi_operation, *result, ScalarType::Integer(*target_type))
        }
        AbstractOperation::ExactIntegerAdd {
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
        } => {
            if checker.available.is_none_or(|sources| {
                [left, right].iter().any(|operand| {
                    !sources.iter().any(|(value, source)| {
                        value == *operand
                            && source.scalar_type() == ScalarType::Integer(*scalar_type)
                    })
                })
            }) {
                return Err(invalid);
            }
            (*psi_operation, *result, ScalarType::Integer(*scalar_type))
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
        _ => return Err(invalid),
    };
    if result_home.defining_operation != operation
        || result_home.source_value != value
        || result_home.scalar_type != scalar_type
        || Some(result_home.shape) != scalar_shape(scalar_type)
    {
        return Err(invalid);
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
        return Err(invalid);
    }
    Ok(())
}
