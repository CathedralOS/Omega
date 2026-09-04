//! Optimizer module role: stage group. Independent keyed requirement replay.

mod ordinary;
mod state;
mod structural;
mod work;
mod writes;

use std::collections::BTreeSet;

use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::StagedOptimizedRegisterHomes;
use omega_target_to_register_environment::{
    FrameAbiPreservationConvention, selected_abi_preservation,
};

use super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPolicy,
    FunctionAllocatedCalleeSavedRequirements,
};
use state::{ReplayTraversal, keyed_homes};

pub(super) struct ReplayResult {
    pub(super) abi: FrameAbiPreservationConvention,
    pub(super) callee_saved_units: Vec<omega_register_model::RegisterUnitId>,
    pub(super) functions: Vec<FunctionAllocatedCalleeSavedRequirements>,
    pub(super) usage: OptimizationWorkUsage,
}

pub(super) fn reconstruct(
    source: &StagedOptimizedRegisterHomes,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReplayResult, AllocatedCalleeSavedRequirementError> {
    if policy
        != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
    {
        return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
    }
    let selected_stage = source
        .legality_stage()
        .live_range_stage()
        .liveness_stage()
        .selected_stage();
    let selected = selected_stage.selected().plan();
    let environment = selected_stage.register_environment();
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
    if !selected.projected_structural_call_returns.is_empty() {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    let callee_saved = preservation
        .convention
        .callee_saved
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let ordinary_homes = keyed_homes(&source.homes().plan().functions)?;
    let structural_homes = keyed_homes(&source.homes().plan().structural_unit_functions)?;
    if ordinary_homes.len() != selected.functions.len()
        || structural_homes.len() != selected.structural_unit_functions.len()
    {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    let mut traversal = ReplayTraversal::new(environment.physical(), &callee_saved);
    for function in &selected.functions {
        let homes = ordinary_homes
            .get(&function.machine)
            .ok_or(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch)?;
        ordinary::reconstruct(&mut traversal, function, homes)?;
    }
    for function in &selected.structural_unit_functions {
        let homes = structural_homes
            .get(&function.machine)
            .ok_or(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch)?;
        structural::reconstruct(&mut traversal, function, homes)?;
    }
    let usage = work::usage(&traversal)?;
    if !usage.within(budget) {
        return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(ReplayResult {
        abi: preservation.kind,
        callee_saved_units: preservation.convention.callee_saved.clone(),
        functions: traversal.functions,
        usage,
    })
}
