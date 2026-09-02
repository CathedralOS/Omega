//! Optimizer module role: stage group. Positional preservation-storage derivation.

mod groups;
mod work;

use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    ValidatedAllocatedCalleeSavedRequirements, ValidatedTargetRegisterEnvironment,
    stages::allocation::abi_preservation::{
        selected_abi_preservation, selected_preservation_storage_catalog,
    },
};

use super::{
    NonAuthoritativeCalleeSaveStorageError, NonAuthoritativeCalleeSaveStoragePlan,
    NonAuthoritativeCalleeSaveStoragePolicy,
};

pub(super) fn derive(
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
    let functions = source_plan
        .functions
        .iter()
        .map(|function| groups::derive_function(function, &catalog))
        .collect::<Result<Vec<_>, _>>()?;
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
