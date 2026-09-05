use omega_regalloc::ValidatedSelectedAnalysis;
use omega_selected_instructions_to_register_homes::AllocationSource;

use super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
    ValidatedAllocatedCalleeSavedRequirements, custody, replay,
};

pub fn validate_allocated_callee_saved_requirements(
    source: &impl AllocationSource,
    candidate: AllocatedCalleeSavedRequirementPlan,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let current = source
        .replay_allocation()
        .map_err(AllocatedCalleeSavedRequirementError::Upstream)?;
    let environment = current.register_environment();
    let manifest = current.post_allocation_manifest().record();
    if candidate.selected != current.selected().selected_identity()
        || candidate.homes != current.homes().receipt().identity()
        || candidate.post_allocation_manifest != manifest.identity
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
    {
        return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
    }
    let replayed = replay::reconstruct(&current, candidate.policy, candidate.budget)?;
    if candidate.usage != replayed.usage {
        return Err(AllocatedCalleeSavedRequirementError::UsageMismatch);
    }
    if candidate.abi != replayed.abi
        || candidate.callee_saved_units != replayed.callee_saved_units
        || candidate.functions != replayed.functions
    {
        return Err(AllocatedCalleeSavedRequirementError::NonCanonicalRequirements);
    }
    let receipt = custody::seal(&candidate);
    Ok(ValidatedAllocatedCalleeSavedRequirements {
        plan: candidate,
        receipt,
    })
}
