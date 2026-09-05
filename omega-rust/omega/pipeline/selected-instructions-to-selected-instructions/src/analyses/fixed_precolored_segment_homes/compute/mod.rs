//! Optimizer module role: stage group. Positional segmented-home production.

mod conflicts;
mod domains;
mod functions;
mod placement;
mod roots;
mod work;

use optimization_core::OptimizationWorkBudget;
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

use crate::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomePlan,
    FixedPrecoloredSegmentHomePolicy, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    requirements: &ValidatedFixedPrecoloredSplitRequirements,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomeError> {
    roots::validate(
        ranges,
        legality,
        fixed,
        requirements,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
    )?;
    match policy {
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => {}
    }
    let mut work = work::Work::new();
    let functions = functions::derive(
        &requirements.plan().functions,
        &ranges.plan().functions,
        physical,
        &mut work,
    )?;
    let structural_unit_functions = functions::derive(
        &requirements.plan().structural_unit_functions,
        &ranges.plan().structural_unit_functions,
        physical,
        &mut work,
    )?;
    let usage = work.finish(budget)?;
    Ok(FixedPrecoloredSegmentHomePlan {
        split_requirements: requirements.receipt().identity(),
        fixed_intervals: fixed.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        optimization_unit: ranges.receipt().optimization_unit(),
        fuel_schedule: ranges.receipt().fuel_schedule(),
        target: ranges.plan().target,
        policy,
        budget,
        usage,
        functions,
        structural_unit_functions,
    })
}
