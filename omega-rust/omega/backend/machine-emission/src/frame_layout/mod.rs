#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Target-owned ordinary frame geometry.
//!
//! This backend calculation joins the post-allocation machine plan to validated
//! preservation storage and chooses exact stack-frame coordinates. The result
//! is independently replayed. It does not claim that prologue, epilogue,
//! unwind, probing, or memory-access instructions have been emitted.
//! Durable plans live in `machine_code::storage::frame_layout`; only the
//! independently validated wrappers and implementation state live here.

mod compute;
mod error;
mod identity;
mod model;
mod replay;
mod save_storage;
mod spill_requirements;
mod validation;

pub use error::*;
pub use identity::target_frame_layout_identity;
pub use model::*;
pub use save_storage::*;
pub use spill_requirements::*;
pub use validation::validate_target_frame_layout;

use register_environment::ValidatedTargetRegisterEnvironment;
use register_homes_to_post_allocation_machine::StagedOptimizedPostAllocationMachinePlan;
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    ValidatedAllocatedCalleeSavedRequirements,
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
