//! Direct calls have one transport record. Reconstruct the result and each
//! argument from the source occurrence before making its result available;
//! neither an unused scalar result nor an empty structural roster changes the
//! meaning of the call. Installed origins are checked and rebound by the owner
//! before entering this ordinary-call check.

use super::{
    AbstractOperation, AbstractOperationPlan, LegalizationError, PsiOptimizationFunction,
    PsiOptimizationUnit, Source, TargetOperationPlan, TargetUnitOperation, ValueId, callee_plan,
    scalar_shape,
};
use target_operations::{NativeCallOrigin, TargetUnitScalarHomeRequirement};

pub(super) fn validate(
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    sources: &mut Vec<(ValueId, Source)>,
    custody: &super::super::super::reference_custody::Custody,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let invalid = || LegalizationError::SourceCustodyMismatch;
    let TargetUnitOperation::Call {
        origin: NativeCallOrigin::Authored,
        psi_operation,
        callee,
        call_plan,
        result_home,
        scalar_arguments,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = target
    else {
        return Err(invalid());
    };
    let (actual, called, values, structural_arguments, claims, requirements, crashes, result) =
        match abstracted {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            } => (
                *psi_operation,
                *callee,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations,
                crash_continuations,
                None,
            ),
            AbstractOperation::CallStructuralScalar {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                result,
            } => (
                *psi_operation,
                *callee,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations,
                crash_continuations,
                Some(*result),
            ),
            AbstractOperation::Call {
                psi_operation,
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                result,
                scalar_type,
            } => (
                *psi_operation,
                *callee,
                arguments.as_slice(),
                &[][..],
                &[][..],
                requirement_obligations,
                crash_continuations,
                Some(abstract_operations::AbstractResult {
                    value: *result,
                    scalar_type: *scalar_type,
                }),
            ),
            _ => return Err(invalid()),
        };
    let expected = callee_plan(*callee, native, plan, unit)?;
    let expected_home = result
        .map(|result| {
            let shape = scalar_shape(result.scalar_type).ok_or_else(invalid)?;
            if expected.result.as_ref().map(|placement| placement.shape) != Some(shape) {
                return Err(invalid());
            }
            Ok(TargetUnitScalarHomeRequirement {
                defining_operation: actual,
                source_value: result.value,
                scalar_type: result.scalar_type,
                shape,
            })
        })
        .transpose()?;
    let callee_function = unit
        .functions
        .iter()
        .find(|function| function.machine == *callee)
        .ok_or_else(invalid)?;
    if psi_operation != &actual
        || callee != &called
        || call_plan != &expected
        || *result_home != expected_home
        || result.is_none() != expected.result.is_none()
        || claim_transfers != claims
        || requirement_obligations != requirements
        || crash_continuations != crashes
        || arguments.len() != structural_arguments.len()
        || callee_function.structural_parameters.len() != arguments.len()
        || callee_function.parameters.len() != values.len()
        || scalar_arguments.len() != values.len()
        || expected.parameters.len() != values.len() + arguments.len()
        || scalar_arguments
            .iter()
            .zip(values)
            .zip(&expected.parameters)
            .enumerate()
            .any(|(position, ((argument, value), placement))| {
                argument.parameter_index != position as u32
                    || argument.placement != *placement
                    || !sources.iter().any(|(source, definition)| {
                        source == value && *definition == argument.source
                    })
            })
    {
        return Err(invalid());
    }
    for (position, (argument, semantic)) in arguments.iter().zip(structural_arguments).enumerate() {
        if super::super::super::structural_call::argument_at(
            semantic,
            position,
            *psi_operation,
            optimized,
            callee_function,
            &expected,
            native,
            plan,
            custody,
        )? != *argument
        {
            return Err(invalid());
        }
    }
    if let Some(home) = expected_home {
        sources.push((home.source_value, Source::Home(home)));
    }
    Ok(())
}
