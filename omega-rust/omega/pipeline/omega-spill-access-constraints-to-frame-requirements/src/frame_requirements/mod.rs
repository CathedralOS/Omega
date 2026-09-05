//! Optimizer module role: executable entrance. Non-authoritative spill-frame requirements.
//!
//! This join authenticates abstract spill-access custody against the selected
//! register environment, derives requirements, and admits them through an
//! independent replay. It chooses no frame layout or executable operation.

mod compute;
mod custody;
mod identity;
mod model;
mod replay;
mod validation;

pub use identity::non_authoritative_spill_frame_requirement_identity;
pub use model::*;
pub use validation::validate_non_authoritative_spill_frame_requirements;

#[cfg(test)]
pub(crate) use compute::derive_zero_access_requirement_for_test;
#[cfg(test)]
pub(crate) use replay::replay_zero_access_requirement_for_test;

use omega_optimization_core::OptimizationWorkBudget;
use omega_selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints;

use crate::ValidatedTargetRegisterEnvironment;

pub fn stage_non_authoritative_spill_frame_requirements(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeSpillFrameRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ValidatedNonAuthoritativeSpillFrameRequirements, SpillFrameRequirementError> {
    let plan = compute::derive(source, environment, policy, budget)?;
    validate_non_authoritative_spill_frame_requirements(source, environment, plan)
}
