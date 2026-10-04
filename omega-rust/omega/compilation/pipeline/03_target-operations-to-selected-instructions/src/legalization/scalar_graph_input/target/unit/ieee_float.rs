use super::super::super::{LegalizationError, ScalarType, ValueShape};
use super::super::{AbstractOperation, TargetUnitOperation, ValueId};
use super::Source;
use semantic_vocabulary::IeeeFloatFormat;
use target_operations::{TargetIeeeFloatFmaOperand, TargetUnitScalarHomeRequirement};

/// Replay IEEE operations against their exact ordered, available scalar sources.
pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
) -> Result<(), LegalizationError> {
    match (target, abstracted) {
        (
            TargetUnitOperation::FloatResultPolicy {
                psi_operation,
                result_home,
                format,
                policy,
            },
            AbstractOperation::FloatResultPolicy {
                psi_operation: expected_operation,
                result,
                format: expected_format,
                policy: expected_policy,
            },
        ) => {
            let scalar_type = ScalarType::IeeeFloat(*expected_format);
            let shape = ValueShape::float(match expected_format {
                IeeeFloatFormat::Binary32 => 4,
                IeeeFloatFormat::Binary64 => 8,
            });
            if psi_operation != expected_operation
                || result_home.defining_operation != *expected_operation
                || result_home.source_value != *result
                || result_home.scalar_type != scalar_type
                || result_home.shape != shape
                || format != expected_format
                || policy.semantic_policy() != *expected_policy
                || policy.operands().any(|operand| {
                    operand.scalar_type() != scalar_type
                        || !sources
                            .iter()
                            .filter(|(value, _)| *value == operand.source_value())
                            .map(|(_, source)| source)
                            .eq(std::iter::once(&operand))
                })
            {
                return Err(LegalizationError::custody());
            }
            sources.push((*result, Source::Home(*result_home)));
        }
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
                return Err(LegalizationError::custody());
            }
            sources.push((*result, Source::Home(*result_home)));
        }
        // A re-view's home is the Terminal operation's result at the target
        // carrier, and its operand is the one available source of the
        // operand value at the source carrier; the pair itself is replayed.
        (
            TargetUnitOperation::ScalarRepresentationView {
                result_home,
                operand,
            },
            AbstractOperation::ScalarRepresentationView {
                psi_operation,
                result,
                source_type,
                target_type,
                operand: expected_operand,
            },
        ) => {
            if terminal_semantics::ScalarRepresentationViewPair::classify(
                *source_type,
                *target_type,
            )
            .is_none()
                || result_home.defining_operation != *psi_operation
                || result_home.source_value != *result
                || result_home.scalar_type != *target_type
                || Some(result_home.shape) != super::super::super::scalar_shape(*target_type)
                || operand.source_value() != *expected_operand
                || operand.scalar_type() != *source_type
                || !sources
                    .iter()
                    .filter(|(value, _)| value == expected_operand)
                    .map(|(_, source)| source)
                    .eq(std::iter::once(operand))
            {
                return Err(LegalizationError::custody());
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
            // Each operand names one exact preceding IEEE constant: the
            // retained source must be that constant's own immediate record,
            // so a same-valued or same-typed producer cannot substitute.
            let exact_operand = |operand: &TargetIeeeFloatFmaOperand, expected: &ValueId| {
                operand.source_value == *expected
                    && sources
                        .iter()
                        .filter(|(value, _)| value == expected)
                        .map(|(_, source)| source)
                        .eq(std::iter::once(&Source::IeeeFloatImmediate {
                            defining_operation: operand.defining_operation,
                            source_value: operand.source_value,
                            value: operand.value,
                        }))
            };
            if psi_operation != expected_operation
                || result != expected_result
                || format != expected_format
                || settlement.terminal_operation != *expected_operation
                || settlement.format != *expected_format
                || !exact_operand(left, expected_left)
                || !exact_operand(right, expected_right)
                || !exact_operand(addend, expected_addend)
            {
                return Err(LegalizationError::custody());
            }
            sources.push((
                *result,
                Source::Home(TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: ScalarType::IeeeFloat(*format),
                    shape: ValueShape::float(match format {
                        IeeeFloatFormat::Binary32 => 4,
                        IeeeFloatFormat::Binary64 => 8,
                    }),
                }),
            ));
        }
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}
