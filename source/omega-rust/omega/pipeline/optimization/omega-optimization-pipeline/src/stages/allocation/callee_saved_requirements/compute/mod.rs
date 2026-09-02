//! Optimizer module role: stage group. Direct positional requirement traversal.

mod ordinary;
mod state;
mod structural;
mod work;

use std::collections::BTreeSet;

use omega_optimization_core::OptimizationWorkBudget;

use crate::{
    StagedOptimizedRegisterHomes, stages::allocation::abi_preservation::selected_abi_preservation,
};

use super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy,
};
use state::DirectTraversal;

pub(super) fn derive(
    source: &StagedOptimizedRegisterHomes,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementError> {
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
    let selected = selected_stage.selected();
    let environment = selected_stage.register_environment();
    let homes = source.homes();
    let manifest = source.post_allocation_manifest().record();
    if source.custody().selected() != selected.receipt().identity()
        || source.custody().homes() != homes.receipt().identity()
        || source.custody().post_allocation_manifest() != manifest.identity
        || source.custody().register_environment() != environment.identity()
        || manifest.selected != selected.receipt().identity()
        || manifest.homes != homes.receipt().identity()
        || manifest.register_environment != environment.identity()
        || manifest.target != environment.target()
    {
        return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
    }
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
    if !selected.plan().projected_structural_call_returns.is_empty() {
        return Err(AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
    }
    let callee_saved = preservation
        .convention
        .callee_saved
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut traversal = DirectTraversal::new(environment.physical(), &callee_saved);
    ordinary::derive(
        &mut traversal,
        &selected.plan().functions,
        &homes.plan().functions,
    )?;
    structural::derive(
        &mut traversal,
        &selected.plan().structural_unit_functions,
        &homes.plan().structural_unit_functions,
    )?;
    let usage = work::usage(&traversal)?;
    if !usage.within(budget) {
        return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(AllocatedCalleeSavedRequirementPlan {
        selected: selected.receipt().identity(),
        homes: homes.receipt().identity(),
        post_allocation_manifest: manifest.identity,
        register_environment: environment.identity(),
        physical_register_model: environment.physical().identity(),
        target: environment.target(),
        abi: preservation.kind,
        callee_saved_units: preservation.convention.callee_saved.clone(),
        policy,
        budget,
        usage,
        functions: traversal.functions,
    })
}
