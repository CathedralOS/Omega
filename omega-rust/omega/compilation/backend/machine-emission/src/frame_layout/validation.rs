use crate::frame_layout::{
    StagedOptimizedPostAllocationMachinePlan, ValidatedAllocatedCalleeSavedRequirements,
    ValidatedNonAuthoritativeCalleeSaveStorage, ValidatedTargetRegisterEnvironment,
};

use super::{
    TargetFrameLayoutError, TargetFrameLayoutPlan, ValidatedTargetFrameLayout, replay, seal,
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
    replay::validate_layout(machine, requirements, storage, environment, &candidate)?;
    let receipt = seal(&candidate);
    Ok(ValidatedTargetFrameLayout {
        plan: std::sync::Arc::new(candidate),
        receipt,
    })
}
