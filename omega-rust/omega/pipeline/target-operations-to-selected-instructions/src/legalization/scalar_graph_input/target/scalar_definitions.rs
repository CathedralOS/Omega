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
