use super::super::super::{LegalizationError, ScalarType, ValueShape};
use super::super::{AbstractOperation, TargetUnitOperation, ValueId};
use super::Source;

/// Replay IEEE operations against their exact ordered, available scalar sources.
pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    match (target, abstracted) {
        (
            TargetUnitOperation::IeeeFloatCompare {
                result_home,
                comparison,
                format,
                left,
                right,
            },
            AbstractOperation::IeeeFloatCompare {
                psi_operation,
                result,
                comparison: expected_comparison,
                format: expected_format,
                left: expected_left,
                right: expected_right,
            },
        ) => {
            if result_home.defining_operation != *psi_operation
                || result_home.source_value != *result
                || result_home.scalar_type != ScalarType::Boolean
                || result_home.shape != ValueShape::integer(1, 1)
                || comparison != expected_comparison
                || format != expected_format
                || left.source_value() != *expected_left
                || right.source_value() != *expected_right
                || left.scalar_type() != ScalarType::IeeeFloat(*format)
                || right.scalar_type() != ScalarType::IeeeFloat(*format)
                || !sources
                    .iter()
                    .filter(|(value, _)| value == expected_left)
                    .map(|(_, source)| source)
                    .eq(std::iter::once(left))
                || !sources
                    .iter()
                    .filter(|(value, _)| value == expected_right)
                    .map(|(_, source)| source)
                    .eq(std::iter::once(right))
            {
                return Err(invalid);
            }
            sources.push((*result, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::IeeeFloatConstant {
                psi_operation,
                result,
                value,
            },
            AbstractOperation::IeeeFloatConstant {
                psi_operation: expected_operation,
                result: expected_result,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && result == expected_result
            && value == expected_value =>
        {
            sources.push((
                *result,
                Source::IeeeFloatImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation,
                result,
                format,
                left,
                right,
                addend,
                settlement,
            },
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                psi_operation: expected_operation,
                result: expected_result,
                format: expected_format,
                left: expected_left,
                right: expected_right,
                addend: expected_addend,
            },
        ) => {
            // Each operand must be the exact immediate source the target row
            // names — a same-valued constant from another definition is a
            // substituted input, not the checked abstract operand.
            let operand_replays = |operand: &target_operations::TargetIeeeFloatFmaOperand,
                                   expected: &ValueId| {
                operand.source_value == *expected
                    && sources.iter().any(|(value, source)| {
                        value == expected
                            && matches!(
                                source,
                                Source::IeeeFloatImmediate {
                                    defining_operation,
                                    source_value,
                                    value: immediate,
                                } if *defining_operation == operand.defining_operation
                                    && *source_value == operand.source_value
                                    && *immediate == operand.value
                            )
                    })
            };
            if psi_operation != expected_operation
                || result != expected_result
                || format != expected_format
                || settlement.terminal_operation != *psi_operation
                || settlement.format != *format
                || !operand_replays(left, expected_left)
                || !operand_replays(right, expected_right)
                || !operand_replays(addend, expected_addend)
            {
                return Err(invalid);
            }
            let Some(shape) = super::super::super::scalar_shape(ScalarType::IeeeFloat(*format))
            else {
                return Err(invalid);
            };
            sources.push((
                *result,
                Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: ScalarType::IeeeFloat(*format),
                    shape,
                }),
            ));
        }
        _ => return Err(invalid),
    }
    Ok(())
}
