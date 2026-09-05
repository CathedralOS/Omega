//! Optimizer module role: stage group. Independent keyed storage reconstruction.

mod groups;
mod work;

use optimization_core::OptimizationWorkBudget;

use crate::save_storage::{
    ValidatedAllocatedCalleeSavedRequirements, ValidatedTargetRegisterEnvironment,
};
use register_environment::{selected_abi_preservation, selected_preservation_storage_catalog};

use super::{
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy,
};

pub(super) fn reconstruct(
    source: &ValidatedAllocatedCalleeSavedRequirements,
    environment: &ValidatedTargetRegisterEnvironment,
    policy: NonAuthoritativeCalleeSaveStoragePolicy,
    budget: OptimizationWorkBudget,
) -> Result<NonAuthoritativeCalleeSaveStoragePlan, NonAuthoritativeCalleeSaveStorageError> {
    if policy != NonAuthoritativeCalleeSaveStoragePolicy::CanonicalTargetPreservationGroupsV1 {
        return Err(NonAuthoritativeCalleeSaveStorageError::UnsupportedPolicy);
    }
    let source_plan = source.plan();
    if source_plan.register_environment != environment.identity()
        || source_plan.physical_register_model != environment.physical().identity()
        || source_plan.target != environment.target()
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    let catalog = selected_preservation_storage_catalog(environment)
        .map_err(|_| NonAuthoritativeCalleeSaveStorageError::UnsupportedTargetCatalog)?;
    if source_plan.abi != preservation.kind
        || source_plan.callee_saved_units != preservation.convention.callee_saved
    {
        return Err(NonAuthoritativeCalleeSaveStorageError::RootMismatch);
    }
    let functions = groups::reconstruct_functions(source_plan, &catalog)?;
    let usage = work::usage(source_plan, &catalog, &functions)?;
    if !usage.within(budget) {
        return Err(NonAuthoritativeCalleeSaveStorageError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(NonAuthoritativeCalleeSaveStoragePlan {
        callee_saved_requirements: source.receipt().identity(),
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        preservation_storage_catalog: catalog.identity(),
        target: environment.target(),
        abi: source_plan.abi,
        callee_saved_units: source_plan.callee_saved_units.clone(),
        policy,
        budget,
        usage,
        functions,
    })
}
