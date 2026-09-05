//! Optimizer module role: stage group. Independently keyed split replay.

mod cuts;
mod function;
mod indexes;
mod partition;
mod topology;
mod work;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};

use crate::{
    FixedPrecoloredSplitRequirementError, FixedPrecoloredSplitRequirementPolicy,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSplitRequirements, FunctionLiveRanges, ValidatedAllocationLegality,
    ValidatedFixedPrecoloredIntervals, ValidatedLiveRanges,
};

pub(super) struct ReplayedSplitRequirements {
    pub(super) functions: Vec<FunctionFixedPrecoloredSplitRequirements>,
    pub(super) structural_unit_functions: Vec<FunctionFixedPrecoloredSplitRequirements>,
    pub(super) usage: OptimizationWorkUsage,
}

pub(super) fn replay(
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    fixed: &ValidatedFixedPrecoloredIntervals,
    policy: FixedPrecoloredSplitRequirementPolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReplayedSplitRequirements, FixedPrecoloredSplitRequirementError> {
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
    Ok(ReplayedSplitRequirements {
        functions,
        structural_unit_functions,
        usage,
    })
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
    let legality = indexes::legality(legality)?;
    let fixed = indexes::fixed(fixed)?;
    ranges
        .iter()
        .enumerate()
        .map(|(function, ranges)| {
            let legality = legality
                .get(&ranges.machine)
                .ok_or(FixedPrecoloredSplitRequirementError::FunctionMismatch { function })?;
            let fixed = fixed
                .get(&ranges.machine)
                .ok_or(FixedPrecoloredSplitRequirementError::FunctionMismatch { function })?;
            function::reconstruct(function, ranges, legality, fixed, work)
        })
        .collect()
}
