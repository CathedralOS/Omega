use crate::{StagedOptimizedRegisterHomes, validate_optimized_register_home_custody};

use super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
    ValidatedAllocatedCalleeSavedRequirements, custody, replay,
};

pub fn validate_allocated_callee_saved_requirements(
    source: &StagedOptimizedRegisterHomes,
    candidate: AllocatedCalleeSavedRequirementPlan,
) -> Result<ValidatedAllocatedCalleeSavedRequirements, AllocatedCalleeSavedRequirementError> {
    let upstream = validate_optimized_register_home_custody(
        source.legality_stage(),
        source.homes(),
        source.post_allocation_manifest(),
    )
    .map_err(AllocatedCalleeSavedRequirementError::Upstream)?;
    let selected_stage = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let environment = selected_stage.register_environment();
    let manifest = source.post_allocation_manifest().record();
    if upstream != source.custody()
        || candidate.selected != selected_stage.selected().receipt().identity()
        || candidate.homes != source.homes().receipt().identity()
        || candidate.post_allocation_manifest != manifest.identity
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
    {
        return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
    }
    let replayed = replay::reconstruct(source, candidate.policy, candidate.budget)?;
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
