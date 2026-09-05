use omega_selected_instructions_to_register_homes::ValidatedAbstractSpillAccessConstraints;

use crate::ValidatedTargetRegisterEnvironment;

use super::{
    NonAuthoritativeSpillFrameRequirementPlan, SpillFrameRequirementError,
    ValidatedNonAuthoritativeSpillFrameRequirements, custody, replay,
};

pub fn validate_non_authoritative_spill_frame_requirements(
    source: &ValidatedAbstractSpillAccessConstraints,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: NonAuthoritativeSpillFrameRequirementPlan,
) -> Result<ValidatedNonAuthoritativeSpillFrameRequirements, SpillFrameRequirementError> {
    if candidate.abstract_spill_access_constraints != source.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.register_environment != source.receipt().register_environment()
        || candidate.target != environment.target()
    {
        return Err(SpillFrameRequirementError::RootMismatch);
    }
    let (functions, usage) =
        replay::reconstruct(source, environment, candidate.policy, candidate.budget)?;
    if candidate.usage != usage {
        return Err(SpillFrameRequirementError::UsageMismatch);
    }
    if candidate.functions != functions {
        return Err(SpillFrameRequirementError::NonCanonicalRequirements);
    }
    let receipt = custody::seal(&candidate);
    Ok(ValidatedNonAuthoritativeSpillFrameRequirements {
        plan: candidate,
        receipt,
    })
}
