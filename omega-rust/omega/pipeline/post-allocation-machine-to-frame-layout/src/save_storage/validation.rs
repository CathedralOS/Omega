use crate::save_storage::{
    ValidatedAllocatedCalleeSavedRequirements, ValidatedTargetRegisterEnvironment,
};
use target_to_register_environment::selected_preservation_storage_catalog;

use super::{
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePlan,
    ValidatedNonAuthoritativeCalleeSaveStorage, custody, replay,
};

pub fn validate_non_authoritative_callee_save_storage(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    candidate: NonAuthoritativeCalleeSaveStoragePlan,
) -> Result<ValidatedNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError> {
    let source_plan = source.plan();
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if candidate.callee_saved_requirements != source.receipt().identity()
        || candidate.register_environment != environment.identity()
        || candidate.physical_register_model != environment.physical().identity()
        || candidate.preservation_storage_catalog != catalog.identity()
        || candidate.target != environment.target()
        || candidate.abi != source_plan.abi
        || candidate.callee_saved_units != source_plan.callee_saved_units
        || source_plan.register_environment != environment.identity()
        || source_plan.physical_register_model != environment.physical().identity()
        || source_plan.target != environment.target()
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let replayed = replay::reconstruct(source, environment, candidate.policy, candidate.budget)?;
    if candidate.usage != replayed.usage {
        return Err(NonAuthoritativeCalleeSaveStorageError::UsageMismatch);
    }
    if candidate.functions != replayed.functions {
        return Err(NonAuthoritativeCalleeSaveStorageError::NonCanonicalStorage);
    }
    let receipt = custody::seal(&candidate);
    Ok(ValidatedNonAuthoritativeCalleeSaveStorage {
        plan: candidate,
        receipt,
    })
}
