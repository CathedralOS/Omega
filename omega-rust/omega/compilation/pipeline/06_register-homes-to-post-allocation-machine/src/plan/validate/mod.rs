//! Optimizer module role: executable entrance. Independent post-allocation plan replay.
//!
//! Validation proceeds in visible rejection order: custody roots, ordinary
//! functions, canonical identity, then receipt.
//! Each child reconstructs from validated inputs and never calls production
//! construction.

mod instruction;
mod ordinary;
mod roots;

use super::post_allocation_receipt;
use crate::{PostAllocationMachineError, ValidatedPostAllocationMachinePlan};
use physical_instructions::PostAllocationMachinePlan;

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_post_allocation_machine_plan(
    plan: PostAllocationMachinePlan,
) -> Result<ValidatedPostAllocationMachinePlan, PostAllocationMachineError> {
    let receipt = post_allocation_receipt(&plan)?;
    Ok(ValidatedPostAllocationMachinePlan::new(plan, receipt))
}
