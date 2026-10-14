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
    let TargetUnitOperation::Call {
        origin: NativeCallOrigin::Authored,
        psi_operation,
        target: callee,
        call_plan,
        result,
        scalar_arguments,
        arguments,
        placed_bindings,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = target
    else {
        return Err(LegalizationError::custody());
    };
    let (actual, called, values, structural_arguments, claims, requirements, crashes) =
        match abstracted {
            AbstractOperation::CallStructural {
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
                abstract_operations::AbstractCallTarget::Machine(*callee),
                arguments.as_slice(),
                &[][..],
                &[][..],
                requirement_obligations,
                crash_continuations,
            ),
            _ => return Err(LegalizationError::custody()),
        };
    let expected = callee_plan(called, native, plan, unit)?;
    let signature = super::super::super::call_signature::signature(called, plan)?;
    let callee_function = called.machine().and_then(|machine| unit.function(machine));
    if psi_operation != &actual
        || *callee != super::super::super::call_signature::native_target(called)
        || call_plan != &expected
        || claim_transfers != claims
        || requirement_obligations != requirements
        || crash_continuations != crashes
        || arguments.len() != structural_arguments.len()
        || signature.structural_parameter_count() != arguments.len()
        || signature.scalar_parameter_count() != values.len()
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
                    || Some(argument.source.scalar_type()) != signature.scalar_parameter(position)
                    || !sources.iter().any(|(source, definition)| {
                        source == value && *definition == argument.source
                    })
            })
    {
        return Err(LegalizationError::custody());
    }
    if callee_function.is_none()
        && (0..signature.structural_parameter_count()).any(|position| {
            signature
                .structural_parameter(position)
                .is_some_and(|formal| {
                    plan.structural_types
                        .placed_views()
                        .iter()
                        .any(|plan| plan.carrier == formal.structural_type)
                })
        })
    {
        return Err(LegalizationError::custody());
    }
    let expected_bindings = if let Some(callee) = callee_function {
        super::super::super::reference_custody::placed_call_custody(
            optimized,
            callee,
            &plan.structural_types,
            structural_arguments,
            custody,
        )?
    } else {
        Vec::new()
    };
    if placed_bindings != &expected_bindings {
        return Err(LegalizationError::custody());
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
            signature,
            &expected,
            native,
            plan,
            custody,
        )? != *argument
        {
            return Err(LegalizationError::custody());
        }
    }
    if let Some(selector) = called.selector() {
        if !sources.iter().any(|(value, source)| {
            *value == selector && source.scalar_type() == target_operations::selector_carrier_type()
        }) {
            return Err(LegalizationError::custody());
        }
    }
    if let Some(home) = result.scalar_home() {
        sources.push((home.source_value, Source::Home(*home)));
    }
    Ok(())
}
