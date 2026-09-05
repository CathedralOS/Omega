//! Optimizer module role: executable entrance. Stage entrance: validate the target-operation roster, assign each function,
//! and retain the source plan identity in the assigned result.

mod cleanup;
mod control;
mod expressions;
mod function;
mod native_callback;
mod placement;
pub(crate) mod shared;

use shared::*;

pub fn assign_registers(
    plan: &TargetOperationPlan,
) -> Result<AssignedOperationPlan, AssignmentError> {
    assign_registers_inner(plan, &[])
}

fn assign_registers_inner(
    plan: &TargetOperationPlan,
    native_callbacks: &[target_operations::TargetNativeCallbackArgument],
) -> Result<AssignedOperationPlan, AssignmentError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(AssignmentError::EntryFunctionMissing(plan.entry));
    }
    Ok(AssignedOperationPlan {
        psi: plan.psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| function::assign_function(function, plan.target, native_callbacks))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

/// Assign one target plan while preserving exact native-only callback
/// arguments separately from semantic scalar values.
pub fn assign_registers_with_native_callbacks(
    input: &target_operations::TargetOperationPlanWithNativeCallbacks,
) -> Result<assigned_target_operations::AssignedOperationPlanWithNativeCallbacks, AssignmentError> {
    let native_callback_arguments =
        native_callback::assign(&input.plan, &input.native_callback_arguments)?;
    let plan = assign_registers_inner(&input.plan, &input.native_callback_arguments)?;
    Ok(
        assigned_target_operations::AssignedOperationPlanWithNativeCallbacks {
            plan,
            native_callback_arguments,
        },
    )
}
