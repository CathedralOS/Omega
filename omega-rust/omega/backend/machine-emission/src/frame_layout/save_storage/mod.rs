//! Optimizer module role: executable entrance. Non-authoritative callee-save storage.
//!
//! This boundary maps validated modified ABI-preserved units through the
//! target-owned storage catalog, assigns canonical abstract area-relative
//! slots, and admits the result only after independent keyed replay. It grants
//! no frame, save/restore instruction, memory, unwind, or publication authority.

mod compute;
mod custody;
mod error;
mod identity;
mod model;
mod replay;
mod validation;

pub use error::*;
pub use identity::non_authoritative_callee_save_storage_identity;
pub use model::*;
pub use validation::validate_non_authoritative_callee_save_storage;

use optimization_core::OptimizationWorkBudget;

use register_environment::{FrameAbiPreservationConvention, ValidatedTargetRegisterEnvironment};
use selected_instructions_to_register_homes::{
    AllocatedCalleeSavedFunctionKind, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedUnitRequirement,
    CalleeSavedModificationWitness, FunctionAllocatedCalleeSavedRequirements,
    ValidatedAllocatedCalleeSavedRequirements,
};

pub fn stage_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let plan = compute::derive(source, environment, policy, budget)?;
    validate_non_authoritative_callee_save_storage(source, environment, plan)
}
