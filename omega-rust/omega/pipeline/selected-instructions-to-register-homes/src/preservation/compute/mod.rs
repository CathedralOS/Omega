//! Optimizer module role: stage group. Direct positional requirement traversal.

mod ordinary;
mod state;
mod work;

use std::collections::BTreeSet;

use optimization_core::OptimizationWorkBudget;

use crate::AllocationOutput;
use crate::ValidatedSelectedAnalysis;
use register_environment::selected_abi_preservation;

use super::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementPlan,
    AllocatedCalleeSavedRequirementPolicy,
};
use state::DirectTraversal;

pub(super) fn derive(
    source: &AllocationOutput<'_>,
    policy: AllocatedCalleeSavedRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementError> {
    if policy
        != AllocatedCalleeSavedRequirementPolicy::AllocatedSelectedWritesIntersectAbiPreservationV1
    {
        return Err(AllocatedCalleeSavedRequirementError::UnsupportedPolicy);
    }
    let selected = source.selected();
    let environment = source.register_environment();
    let homes = source.homes();
    let manifest = source.post_allocation_manifest().record();
    if manifest.selected != selected.selected_identity()
        || manifest.homes != homes.receipt().identity()
        || manifest.register_environment != environment.identity()
        || manifest.target != environment.target()
    {
        return Err(AllocatedCalleeSavedRequirementError::RootMismatch);
    }
    let preservation = selected_abi_preservation(environment)
        .map_err(|_| AllocatedCalleeSavedRequirementError::UnsupportedTargetConvention)?;
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
    let usage = work::usage(&traversal)?;
    if !usage.within(budget) {
        return Err(AllocatedCalleeSavedRequirementError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(AllocatedCalleeSavedRequirementPlan {
        selected: selected.selected_identity(),
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
