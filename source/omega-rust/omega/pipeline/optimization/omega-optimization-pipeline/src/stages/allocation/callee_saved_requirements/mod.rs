//! Optimizer module role: executable entrance. Allocation-visible ABI preservation requirements.
//!
//! This baseline-home boundary reports selected writes intersecting the exact
//! ABI callee-saved roster, then admits them only after independent keyed
//! replay. It grants no save/restore, frame, unwind, or publication authority.

mod compute;
mod custody;
mod error;
mod identity;
mod model;
mod replay;
mod validation;

pub use error::*;
pub use identity::allocated_callee_saved_requirement_identity;
pub use model::*;
pub use validation::validate_allocated_callee_saved_requirements;

use omega_optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedRegisterHomes;

pub fn stage_allocated_callee_saved_requirements(
    source: &StagedOptimizedRegisterHomes,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let plan = compute::derive(source, policy, budget)?;
    validate_allocated_callee_saved_requirements(source, plan)
}
