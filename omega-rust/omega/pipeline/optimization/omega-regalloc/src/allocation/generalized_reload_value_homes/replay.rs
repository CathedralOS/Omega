//! Independent point-indexed replay of both generalized reload homes.

mod homes;
mod roots;
mod sources;
mod timeline;
mod work;

use omega_optimization_core::OptimizationWorkBudget;
use omega_register_model::{
    RegisterClassId, RegisterViewId, TargetRegisterEnvironmentConstraintKeys,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_selected_instructions::VirtualRegisterId;
use omega_target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomePolicy, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    LiveRangePoint, ValidatedAbstractSpillInsertion, ValidatedAllocationLegality,
    ValidatedGeneralizedSpillInsertion, ValidatedLiveRanges, ValidatedSpillRecoveryActions,
};

#[derive(Clone)]
struct ReplaySpec {
    action: GeneralizedSpillActionId,
    source: GeneralizedSpillActionSource,
    block: omega_selected_instructions::SelectedBlockId,
    store_point: LiveRangePoint,
    start: LiveRangePoint,
    exclusive_end: LiveRangePoint,
    class: RegisterClassId,
    candidates: Vec<RegisterViewId>,
    victim: VirtualRegisterId,
    victim_view: RegisterViewId,
    before_reload: Option<GeneralizedSpillActionId>,
}

#[derive(Clone, Copy)]
struct Occupant {
    value: GeneralizedReloadCoexistingValue,
    class: RegisterClassId,
    exclusive_end: LiveRangePoint,
    view: RegisterViewId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay(
    generalized: &ValidatedGeneralizedSpillInsertion,
    first: &ValidatedAbstractSpillInsertion,
    second: &ValidatedSpillRecoveryActions,
    selected: &ValidatedSelectedInstructions,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: GeneralizedReloadValueHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<GeneralizedReloadValueHomePlan, GeneralizedReloadValueHomeError> {
    roots::reconstruct(
        generalized,
        first,
        second,
        selected,
        ranges,
        legality,
        physical,
        constraints,
        reservations,
        selected_keys,
        policy,
    )?;
    let mut functions = Vec::with_capacity(generalized.plan().functions.len());
    for function in 0..generalized.plan().functions.len() {
        let generalized_function = &generalized.plan().functions[function];
        let first_function = &first.plan().functions[function];
        let legality_function = &legality.plan().functions[function];
        let ranges_function = &ranges.plan().functions[function];
        if [
            first_function.machine,
            legality_function.machine,
            ranges_function.machine,
        ]
        .into_iter()
        .any(|machine| machine != generalized_function.machine)
        {
            return Err(GeneralizedReloadValueHomeError::FunctionMismatch { function });
        }
        if !(ranges_function.tied_pairs.is_empty() && ranges_function.early_clobbers.is_empty()) {
            return Err(
                GeneralizedReloadValueHomeError::UnsupportedConstraintTopology { function },
            );
        }
        let specs = sources::index(
            function,
            generalized_function,
            first_function,
            second,
            legality_function,
        )?;
        let outcomes = timeline::reconstruct(
            function,
            &specs,
            first_function,
            legality_function,
            ranges_function,
            physical,
        )?;
        functions.push(FunctionGeneralizedReloadValueHomes {
            machine: generalized_function.machine,
            outcomes,
        });
    }
    let usage = work::reconstruct(&functions)?;
    if !usage.within(budget) {
        return Err(GeneralizedReloadValueHomeError::BudgetExceeded {
            required: usage,
            budget,
        });
    }
    Ok(GeneralizedReloadValueHomePlan {
        generalized_spill_insertion: generalized.receipt().identity(),
        abstract_spill_insertion: first.receipt().identity(),
        spill_recovery_actions: second.receipt().identity(),
        selected: selected.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: generalized.plan().register_environment,
        allocator_availability: generalized.plan().allocator_availability,
        optimization_unit: generalized.plan().optimization_unit,
        fuel_schedule: generalized.plan().fuel_schedule,
        policy,
        budget,
        usage,
        functions,
    })
}
