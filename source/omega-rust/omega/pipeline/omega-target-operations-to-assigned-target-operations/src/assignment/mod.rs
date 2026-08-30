//! Optimizer module role: executable entrance. Stage entrance: validate the target-operation roster, assign each function,
//! and retain the source plan identity in the assigned result.

mod cleanup;
mod control;
mod expressions;
mod function;
mod placement;
pub(crate) mod shared;

use shared::*;

pub fn assign_registers(
    plan: &TargetOperationPlan,
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
            .map(|function| function::assign_function(function, plan.target))
            .collect::<Result<Vec<_>, _>>()?,
    })
}
