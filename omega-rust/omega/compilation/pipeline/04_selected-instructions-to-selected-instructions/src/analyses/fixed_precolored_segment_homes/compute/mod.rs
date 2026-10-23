//! Optimizer module role: stage group. Positional segmented-home production.

mod conflicts;

mod functions;
mod placement;
mod roots;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};

use crate::work_accounting::Work;

use crate::{
    FixedPrecoloredSegmentHomeError, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedFixedPrecoloredSplitRequirements,
    ValidatedLiveRanges,
};
use register_homes::{FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePolicy};

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
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
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
    // The stage's exact starting receipt; every charge below is one named
    // visitor event of this pass.
    let mut work = Work::new(OptimizationWorkUsage {
        rule_evaluations: 1,
        validation_steps: 1,
        commits: 1,
        iterations: 1,
        ..Default::default()
    });
    let functions = functions::derive(
        &requirements.plan().functions,
        &ranges.plan().functions,
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
    })
}
