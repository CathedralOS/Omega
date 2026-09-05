//! Canonical producer coordination for generalized reload-home assignment.

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
use selected_instructions::VirtualRegisterId;
use target_operations_to_selected_instructions::ValidatedSelectedInstructions;

use crate::{
    FunctionGeneralizedReloadValueHomes, GeneralizedReloadCoexistingValue,
    GeneralizedReloadValueHomeError, GeneralizedReloadValueHomePlan,
    GeneralizedReloadValueHomePolicy, GeneralizedSpillActionId, GeneralizedSpillActionSource,
    LiveRangePoint, ValidatedAbstractSpillInsertion, ValidatedAllocationLegality,
    ValidatedGeneralizedSpillInsertion, ValidatedLiveRanges, ValidatedSpillRecoveryActions,
};

#[derive(Clone)]
struct ReloadSpec {
    action: GeneralizedSpillActionId,
    source: GeneralizedSpillActionSource,
    block: selected_instructions::SelectedBlockId,
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
struct ActiveHome {
    value: GeneralizedReloadCoexistingValue,
    class: RegisterClassId,
    exclusive_end: LiveRangePoint,
    view: RegisterViewId,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
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
    roots::admit(
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
    let functions = generalized
        .plan()
        .functions
        .iter()
        .zip(&first.plan().functions)
        .zip(&legality.plan().functions)
        .zip(&ranges.plan().functions)
        .enumerate()
        .map(|(function, (((generalized, first), legality), ranges))| {
            if generalized.machine != first.machine
                || generalized.machine != legality.machine
                || generalized.machine != ranges.machine
            {
                return Err(GeneralizedReloadValueHomeError::FunctionMismatch { function });
            }
            if !ranges.tied_pairs.is_empty() || !ranges.early_clobbers.is_empty() {
                return Err(
                    GeneralizedReloadValueHomeError::UnsupportedConstraintTopology { function },
                );
            }
            let specs = sources::reconstruct(function, generalized, first, second, legality)?;
            let outcomes = schedule::assign(function, &specs, first, legality, ranges, physical)?;
            Ok(FunctionGeneralizedReloadValueHomes {
                machine: generalized.machine,
                outcomes,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let usage = work::usage(&functions)?;
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
