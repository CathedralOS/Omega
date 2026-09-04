//! Optimizer module role: executable entrance. Function-relative realization shared by every allocation-recovery rule.
//!
//! [`source`] names the closed recovery taxonomy. Construction and validation
//! independently traverse the selected form owned by that source; this
//! entrance owns their only join.

mod construction;
mod custody;
mod manifest;
mod model;
mod source;
mod validation;

#[cfg(test)]
mod test_support;

pub use model::*;
pub use source::*;
pub use validation::validate_allocation_recovery_function_relative_realization;

#[cfg(test)]
pub(crate) use test_support::*;

use crate::StagedOptimizedPostAllocationMachinePlan;

pub fn stage_allocation_recovery_function_relative_realization(
    source: StagedAllocationRecoveryFunctionRelativeSource,
    machine: StagedOptimizedPostAllocationMachinePlan,
) -> Result<
    StagedAllocationRecoveryFunctionRelativeRealization,
    AllocationRecoveryFunctionRelativeRealizationError,
> {
    let staged = construction::construct(source, machine)?;
    validate_allocation_recovery_function_relative_realization(&staged)?;
    Ok(staged)
}
