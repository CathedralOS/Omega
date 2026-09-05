use optimization_core::OptimizationWorkUsage;
use register_model::ValidatedPreservationStorageCatalog;

use crate::frame_layout::save_storage::AllocatedCalleeSavedRequirementPlan;

use super::super::{
    FunctionNonAuthoritativeCalleeSaveStorage, NonAuthoritativeCalleeSaveStorageError,
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
            checked_add(total, count(group.preserved_units.len())?)
        })?;
    let m = source.functions.iter().try_fold(0_u64, |total, function| {
        checked_add(total, count(function.modified_units.len())?)
    })?;
    let w = source
        .functions
        .iter()
        .flat_map(|function| &function.modified_units)
        .try_fold(0_u64, |total, requirement| {
            checked_add(total, count(requirement.witnesses.len())?)
        })?;
    let s = functions.iter().try_fold(0_u64, |total, function| {
        checked_add(total, count(function.slots.len())?)
    })?;
    Ok(OptimizationWorkUsage {
        rule_evaluations: [1, f, m].into_iter().try_fold(0, checked_add)?,
        candidates: [g, m].into_iter().try_fold(0, checked_add)?,
        validation_steps: [1, g, u, f, m, w, s].into_iter().try_fold(0, checked_add)?,
        commits: [1, f, s, m, w].into_iter().try_fold(0, checked_add)?,
        iterations: [1, g, u, f, m, w].into_iter().try_fold(0, checked_add)?,
    })
}

fn count(value: usize) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    u64::try_from(value).map_err(|_| NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}

fn checked_add(left: u64, right: u64) -> Result<u64, NonAuthoritativeCalleeSaveStorageError> {
    left.checked_add(right)
        .ok_or(NonAuthoritativeCalleeSaveStorageError::WorkOverflow)
}
