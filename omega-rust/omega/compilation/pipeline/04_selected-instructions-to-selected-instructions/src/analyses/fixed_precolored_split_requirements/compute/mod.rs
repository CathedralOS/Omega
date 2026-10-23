//! Optimizer module role: stage group. Positional split-requirement production.

mod cuts;
mod function;
mod partition;
mod topology;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::work_accounting::Work;
use crate::{
    FixedPrecoloredSplitRequirementError, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges,
};
use register_homes::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPolicy, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements,
};
use selected_instructions::FunctionLiveRanges;

pub(super) fn compute(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    policy: FixedPrecoloredSplitRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementError> {
    roots(ranges, legality, fixed)?;
    match policy {
        FixedPrecoloredSplitRequirementPolicy::FixedUseBoundaryRequirementsV1 => {}
    }
    // The stage's exact starting receipt; every charge below is one named
    // visitor event of this pass.
    let mut work = Work::new(OptimizationWorkUsage {
        validation_steps: 1,
        iterations: 2,
        ..Default::default()
    });
    let functions = family(
        &ranges.plan().functions,
        &legality.plan().functions,
        &fixed.plan().functions,
        &mut work,
    )?;
    let usage = work.finish(budget)?;
    Ok(FixedPrecoloredSplitRequirementPlan {
        fixed_intervals: fixed.receipt().identity(),
        ranges: ranges.receipt().identity(),
        legality: legality.receipt().identity(),
        register_environment: legality.receipt().register_environment(),
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

fn roots(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
) -> Result<(), FixedPrecoloredSplitRequirementError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().ranges() != ranges.receipt().identity()
        || fixed.receipt().legality() != legality.receipt().identity()
        || fixed.receipt().register_environment() != legality.receipt().register_environment()
        || fixed.receipt().allocator_availability() != legality.receipt().allocator_availability()
        || fixed.receipt().optimization_unit() != ranges.receipt().optimization_unit()
        || fixed.receipt().fuel_schedule() != ranges.receipt().fuel_schedule()
        || fixed.receipt().policy()
            != FixedPrecoloredIntervalPolicy::FixedConstraintPointIntervalsV1
    {
        return Err(FixedPrecoloredSplitRequirementError::RootMismatch);
    }
    Ok(())
}

fn family(
    ranges: &[std::sync::Arc<FunctionLiveRanges>],
    legality: &[std::sync::Arc<FunctionAllocationLegality>],
    fixed: &[FunctionFixedPrecoloredIntervals],
    work: &mut Work,
) -> Result<Vec<FunctionFixedPrecoloredSplitRequirements>, FixedPrecoloredSplitRequirementError> {
    if ranges.len() != legality.len() || ranges.len() != fixed.len() {
        return Err(FixedPrecoloredSplitRequirementError::RootMismatch);
    }
    ranges
        .iter()
        .zip(legality)
        .zip(fixed)
        .enumerate()
        .map(|(function, ((ranges, legality), fixed))| {
            function::derive(function, ranges, legality, fixed, work)
        })
        .collect()
}
