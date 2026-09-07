use super::*;
use target_operations::{TargetUnitBody, TargetUnitScalarArgumentSource as Source};
pub(super) fn validate(
    body: &TargetUnitBody,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    if body.structural_types != plan.structural_types
        || body.structural_types != unit.structural_types
        || body.operations.len() != abstracted.operations.len()
    {
        return Err(invalid);
    }
    let mut sources = optimized
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            (
                parameter.value,
                Source::Parameter {
                    parameter_index: index as u32,
                    source_value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
            )
        })
        .collect::<Vec<_>>();
    for (target, abstracted) in body.operations.iter().zip(&abstracted.operations) {
        match (target, abstracted) {
            (
                TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type,
                    value,
                },
                AbstractOperation::IntegerConstant {
                    psi_operation: actual,
                    result: defined,
                    scalar_type: integer,
                    value: literal,
                },
            ) if psi_operation == actual
                && result == defined
                && *integer == ScalarType::Integer(*scalar_type)
                && value == literal =>
            {
                sources.push((
                    *result,
                    Source::IntegerImmediate {
                        defining_operation: *psi_operation,
                        source_value: *result,
                        scalar_type: *scalar_type,
                        value: *value,
                    },
                ));
            }
            (
                TargetUnitOperation::ScalarCall {
                    psi_operation,
                    callee,
                    call_plan,
                    result_home,
                    arguments,
                    requirement_obligations,
                    crash_continuations,
                },
                AbstractOperation::Call {
                    psi_operation: actual,
                    result,
                    callee: called,
                    arguments: values,
                    scalar_type,
                    requirement_obligations: requirements,
                    crash_continuations: crashes,
                },
            ) => {
                let expected = callee_plan(*callee, native, plan, unit)?;
                if psi_operation != actual
                    || callee != called
                    || call_plan != &expected
                    || result_home.defining_operation != *actual
                    || result_home.source_value != *result
                    || result_home.scalar_type != *scalar_type
                    || Some(result_home.shape) != expected.result.as_ref().map(|value| value.shape)
                    || requirement_obligations != requirements
                    || crash_continuations != crashes
                    || arguments.len() != values.len()
                    || arguments.len() != expected.parameters.len()
                    || arguments
                        .iter()
                        .zip(values)
                        .zip(&expected.parameters)
                        .enumerate()
                        .any(|(index, ((argument, value), placement))| {
                            argument.parameter_index != index as u32
                                || argument.placement != *placement
                                || !sources.iter().any(|(source, definition)| {
                                    source == value && *definition == argument.source
                                })
                        })
                {
                    return Err(invalid);
                }
                sources.push((*result, Source::Home(*result_home)));
            }
            (
                TargetUnitOperation::Return {
                    psi_edge,
                    cleanup_actions,
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: edge,
                    cleanup_actions: cleanup,
                },
            ) if psi_edge == edge && cleanup_actions == cleanup && cleanup.is_empty() => {}
            _ => return Err(invalid),
        }
    }
    Ok(())
}
