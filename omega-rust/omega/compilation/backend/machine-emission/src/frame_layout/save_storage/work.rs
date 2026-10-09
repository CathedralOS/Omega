use optimization_core::OptimizationWorkUsage;
use register_model::ValidatedPreservationStorageCatalog;

use super::{
    AllocatedCalleeSavedRequirementPlan, FunctionNonAuthoritativeCalleeSaveStorage,
    NonAuthoritativeCalleeSaveStorageError,
};

pub(super) fn usage(
    source: &AllocatedCalleeSavedRequirementPlan,
    catalog: &ValidatedPreservationStorageCatalog,
    functions: &[FunctionNonAuthoritativeCalleeSaveStorage],
) -> Result<OptimizationWorkUsage, NonAuthoritativeCalleeSaveStorageError> {
    let f = count(source.functions.len())?;
    let g = count(catalog.catalog().groups.len())?;
    let u = catalog
        .catalog()
        .groups
        .iter()
        .try_fold(0_u64, |total, group| {
            add(total, count(group.preserved_units.len())?)
        })?;
    let m = source.functions.iter().try_fold(0_u64, |total, function| {
        add(total, count(function.modified_units.len())?)
    })?;
    let w = source
        .modified_units()
        .try_fold(0_u64, |total, requirement| {
            add(total, count(requirement.witnesses.len())?)
        })?;
    let s = functions.iter().try_fold(0_u64, |total, function| {
        add(total, count(function.slots.len())?)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: [1, f, m].into_iter().try_fold(0, add)?,
        candidates: [g, m].into_iter().try_fold(0, add)?,
        validation_steps: [1, g, u, f, m, w, s].into_iter().try_fold(0, add)?,
        commits: [1, f, s, m, w].into_iter().try_fold(0, add)?,
        iterations: [1, g, u, f, m, w].into_iter().try_fold(0, add)?,
    })
}

fn count(value: usize) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    u64::try_from(value).map_err(|_| NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}

fn add(left: u64, right: u64) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    left.checked_add(right)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}

/// The requirement extent the derivation reads: every function, modified
/// unit and modification witness of the source plan, each once. The pass
/// budget's ceilings scale with it (`OptimizationWorkBudget::for_extent`).
pub(super) fn extent(
    source: &AllocatedCalleeSavedRequirementPlan,
) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    source
        .functions
        .iter()
        .try_fold(count(source.functions.len())?, |total, function| {
            function.modified_units.iter().try_fold(
                add(total, count(function.modified_units.len())?)?,
                |total, requirement| add(total, count(requirement.witnesses.len())?),
            )
        })
}
