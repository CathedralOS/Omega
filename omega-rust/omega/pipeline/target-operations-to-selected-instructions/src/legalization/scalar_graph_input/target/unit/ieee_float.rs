use super::{
    AbstractOperation, LegalizationError, ScalarType, Source, TargetUnitOperation, ValueId,
    ValueShape,
};

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
        _ => return Err(invalid),
    }
    Ok(())
}
