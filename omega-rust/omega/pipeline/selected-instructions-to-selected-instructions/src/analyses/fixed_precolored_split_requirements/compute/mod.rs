//! Optimizer module role: stage group. Positional split-requirement production.

mod cuts;
mod function;
mod partition;
mod topology;
mod work;

use optimization_core::OptimizationWorkBudget;

use crate::{
    FixedPrecoloredIntervalPolicy, FixedPrecoloredSplitRequirementError,
    FixedPrecoloredSplitRequirementPlan, FixedPrecoloredSplitRequirementPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSplitRequirements, FunctionLiveRanges, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges,
};

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
    let mut work = work::Work::new();
    let functions = family(
        &ranges.plan().functions,
        &legality.plan().functions,
        &fixed.plan().functions,
        &mut work,
    )?;
    let structural_unit_functions = family(
        &ranges.plan().structural_unit_functions,
        &legality.plan().structural_unit_functions,
        &fixed.plan().structural_unit_functions,
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
        structural_unit_functions,
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
    ranges: &[FunctionLiveRanges],
    legality: &[FunctionAllocationLegality],
    fixed: &[FunctionFixedPrecoloredIntervals],
    work: &mut work::Work,
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
