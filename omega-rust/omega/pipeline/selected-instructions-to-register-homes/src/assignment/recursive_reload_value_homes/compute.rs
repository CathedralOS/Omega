//! Canonical producer coordination using one sorted reload schedule.

mod homes;
mod roots;
mod schedule;
mod sources;
mod work;

use optimization_core::OptimizationWorkBudget;
use register_model::{
    RegisterClassId, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    GeneralizedSpillActionId, LiveRangePoint, RecursiveReloadCoexistingValue,
    RecursiveReloadValueHomeError, RecursiveReloadValueHomePlan, RecursiveReloadValueHomePolicy,
    RecursiveSpillActionSource, ValidatedAllocationLegality, ValidatedGeneralizedReloadValueHomes,
    ValidatedGeneralizedSpillRecoveryActions, ValidatedLiveRanges,
    ValidatedRecursiveSpillInsertion,
};

#[derive(Clone)]
struct ReloadSpec {
    action: GeneralizedSpillActionId,
    source: RecursiveSpillActionSource,
    block: selected_instructions::SelectedBlockId,
    start: LiveRangePoint,
    full_exclusive_end: LiveRangePoint,
    exclusive_end: LiveRangePoint,
    class: RegisterClassId,
    candidates: Vec<RegisterViewId>,
}

#[derive(Clone, Copy)]
struct ActiveHome {
    value: RecursiveReloadCoexistingValue,
    class: RegisterClassId,
    exclusive_end: LiveRangePoint,
    view: RegisterViewId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    recursive: &ValidatedRecursiveSpillInsertion,
    recovery: &ValidatedGeneralizedSpillRecoveryActions,
    prior: &ValidatedGeneralizedReloadValueHomes,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: RecursiveReloadValueHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<RecursiveReloadValueHomePlan, RecursiveReloadValueHomeError> {
    roots::admit(
        recursive,
        recovery,
        prior,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
    )?;
    let mut functions = Vec::with_capacity(recursive.plan().functions.len());
    for function in 0..recursive.plan().functions.len() {
        let recursive_function = &recursive.plan().functions[function];
        let prior_function = &prior.plan().functions[function];
        let legality_function = &legality.plan().functions[function];
        let ranges_function = &ranges.plan().functions[function];
        if recursive_function.machine != prior_function.machine
            || recursive_function.machine != legality_function.machine
            || recursive_function.machine != ranges_function.machine
        {
            return Err(RecursiveReloadValueHomeError::FunctionMismatch { function });
        }
        if !ranges_function.tied_pairs.is_empty() || !ranges_function.early_clobbers.is_empty() {
            return Err(RecursiveReloadValueHomeError::UnsupportedConstraintTopology { function });
        }
        let specs = sources::reconstruct(
            function,
            recursive_function,
            prior_function,
            legality_function,
        )?;
        let assignments = schedule::assign(
            function,
            &specs,
            recursive_function,
            recovery,
            prior_function,
            legality_function,
            physical,
        )?;
        functions.push(crate::FunctionRecursiveReloadValueHomes {
            machine: recursive_function.machine,
            assignments,
        });
    }
    let usage = work::usage(&functions)?;
    if !usage.within(budget) {
        return Err(RecursiveReloadValueHomeError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(RecursiveReloadValueHomePlan {
        recursive_spill_insertion: recursive.receipt().identity(),
        recovery_actions: recovery.receipt().identity(),
        prior_reload_value_homes: prior.receipt().identity(),
        selected: selected.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: recursive.plan().register_environment,
        allocator_availability: recursive.plan().allocator_availability,
        optimization_unit: recursive.plan().optimization_unit,
        fuel_schedule: recursive.plan().fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    })
}
