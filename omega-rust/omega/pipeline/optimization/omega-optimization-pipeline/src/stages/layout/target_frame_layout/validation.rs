use crate::{
    StagedOptimizedPostAllocationMachinePlan, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetRegisterEnvironment,
};

use super::{
    TargetFrameLayoutError, TargetFrameLayoutPlan, ValidatedTargetFrameLayout, compute, seal,
};

pub fn validate_target_frame_layout(
    machine: &StagedOptimizedPostAllocationMachinePlan,
    requirements: &ValidatedAllocatedCalleeSavedRequirements,
    storage: &ValidatedNonAuthoritativeCalleeSaveStorage,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: TargetFrameLayoutPlan,
) -> Result<ValidatedTargetFrameLayout, TargetFrameLayoutError> {
    if candidate.post_allocation_machine != machine.machine().receipt().identity()
        || candidate.callee_saved_requirements != requirements.receipt().identity()
        || candidate.callee_save_storage != storage.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.target != environment.target()
        || candidate.abi != requirements.plan().abi
    {
        return Err(TargetFrameLayoutError::RootMismatch);
    }
    let replayed = compute::derive(
        machine,
        requirements,
        storage,
        environment,
        candidate.policy,
    )?;
    if candidate != replayed {
        return Err(TargetFrameLayoutError::NonCanonicalLayout);
    }
    let receipt = seal(&candidate);
    Ok(ValidatedTargetFrameLayout {
        plan: candidate,
        receipt,
    })
}
