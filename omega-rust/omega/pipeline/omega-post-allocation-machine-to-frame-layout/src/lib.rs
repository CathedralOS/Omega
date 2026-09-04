#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Target-owned ordinary frame geometry.
//!
//! This stage joins the selected post-allocation machine plan to validated
//! preservation storage and chooses exact stack-frame coordinates. The result
//! is independently replayed. It does not claim that prologue, epilogue,
//! unwind, probing, or memory-access instructions have been emitted.

mod compute;
mod error;
mod identity;
mod model;
mod validation;

pub use error::*;
pub use identity::target_frame_layout_identity;
pub use model::*;
pub use validation::validate_target_frame_layout;

use omega_callee_saved_requirements_to_save_storage::{
    NonAuthoritativeCalleeSaveSlotId, NonAuthoritativeCalleeSaveStorageIdentity,
    ValidatedNonAuthoritativeCalleeSaveStorage,
};
use omega_register_homes_to_callee_saved_requirements::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    ValidatedAllocatedCalleeSavedRequirements,
};
use omega_register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use omega_target_to_register_environment::{
    FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment,
};

pub fn stage_target_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: TargetFrameLayoutPolicy,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    let plan = compute::derive(machine, requirements, storage, environment, policy)?;
    validate_target_frame_layout(machine, requirements, storage, environment, plan)
}
