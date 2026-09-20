//! Direct calls have one transport record. Reconstruct the result and each
//! argument from the source occurrence before making its result available;
//! neither an unused scalar result nor an empty structural roster changes the
//! meaning of the call. Installed origins are checked and rebound by the owner
//! before entering this ordinary-call check.

use super::{
    AbstractOperation, AbstractOperationPlan, LegalizationError, PsiOptimizationFunction,
    PsiOptimizationUnit, Source, TargetOperationPlan, TargetUnitOperation, ValueId, callee_plan,
};
use target_operations::{NativeCallOrigin, TargetCallResult};

mod results;

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
        result,
        scalar_arguments,
        arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = target
    else {
        return Err(invalid());
    };
    let (actual, called, values, structural_arguments, claims, requirements, crashes) =
        match abstracted {
            AbstractOperation::CallUnit {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
            }
            | AbstractOperation::CallStructuralScalar {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                callee,
                arguments,
                structural_arguments,
                claim_transfers,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                *psi_operation,
                *callee,
                arguments.as_slice(),
                structural_arguments.as_slice(),
                claim_transfers.as_slice(),
                requirement_obligations,
                crash_continuations,
            ),
            AbstractOperation::Call {
                psi_operation,
                callee,
                arguments,
                requirement_obligations,
                crash_continuations,
                ..
            } => (
                *psi_operation,
                *callee,
                arguments.as_slice(),
                &[][..],
                &[][..],
                requirement_obligations,
                crash_continuations,
            ),
            _ => return Err(invalid()),
        };
    let expected = callee_plan(*callee, native, plan, unit)?;
    let callee_function = unit
        .functions
        .iter()
        .find(|function| function.machine == *callee)
        .ok_or_else(invalid)?;
    if psi_operation != &actual
        || callee != &called
        || call_plan != &expected
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
                    || argument.source.scalar_type()
                        != callee_function.parameters[position].scalar_type
                    || !sources.iter().any(|(source, definition)| {
                        source == value && *definition == argument.source
                    })
            })
    {
        return Err(invalid());
    }
    results::validate(
        result,
        abstracted,
        callee_function,
        &expected,
        custody,
        optimized,
        plan,
    )?;
    // Structural-result calls currently include block-carried byte views
    // but have a narrower primitive projection rule. Retain that admission
    // boundary while sharing the transport checks and argument traversal.
    let reconstruct = if matches!(result, TargetCallResult::Structural { .. }) {
        super::super::super::aggregate_results::call_argument
    } else {
        super::super::super::structural_call::argument_at
    };
    for (position, (argument, semantic)) in arguments.iter().zip(structural_arguments).enumerate() {
        if reconstruct(
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
    if let Some(home) = result.scalar_home() {
        sources.push((home.source_value, Source::Home(*home)));
    }
    Ok(())
}
