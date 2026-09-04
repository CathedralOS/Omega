//! Optimizer module role: executable entrance. Function-relative realization shared by every allocation-recovery rule.
//!
//! Current allocation facts feed one construction and replay path; recovery
//! history remains private to the allocation phase.

mod construction;
mod custody;
mod manifest;
mod model;
mod selection;
mod validation;

#[cfg(test)]
mod test_support;

pub use model::*;
pub use validation::validate_allocation_recovery_function_relative_realization;

#[cfg(test)]
pub(crate) use test_support::*;

use crate::StagedOptimizedPostAllocationMachinePlan;

pub fn stage_allocation_recovery_function_relative_realization<Source>(
    source: Source,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealization,
    AllocationRecoveryFunctionRelativeRealizationError,
>
where
    Source: TryInto<omega_selected_instructions_to_register_homes::RetainedAllocation>,
    omega_selected_instructions_to_register_homes::AllocationReplayError: From<Source::Error>,
{
    let allocation = source.try_into().map_err(|error| match error.into() {
        omega_selected_instructions_to_register_homes::AllocationReplayError::SelectionMismatch => {
            AllocationRecoveryFunctionRelativeRealizationError::UnsupportedSelections
        }
        other => AllocationRecoveryFunctionRelativeRealizationError::Allocation(other),
    })?;
    let staged = construction::construct(allocation, machine)?;
    validate_allocation_recovery_function_relative_realization(&staged)?;
    Ok(staged)
}
