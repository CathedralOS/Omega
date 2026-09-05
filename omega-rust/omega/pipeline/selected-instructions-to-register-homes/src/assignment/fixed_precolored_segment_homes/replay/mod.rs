//! Optimizer module role: stage group. Independently keyed segmented-home replay.

mod conflicts;
mod domains;
mod indexes;
mod placement;
mod work;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;

use crate::{
    FixedPrecoloredSegmentHomeError, FixedPrecoloredSegmentHomePolicy,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
    FunctionLiveRanges,
};

pub(super) struct ReplayedHomes {
    pub(super) functions: Vec<FunctionFixedPrecoloredSegmentHomes>,
    pub(super) structural_unit_functions: Vec<FunctionFixedPrecoloredSegmentHomes>,
    pub(super) usage: OptimizationWorkUsage,
}

pub(super) fn replay(
    ranges: &crate::ValidatedLiveRanges,
    requirements: &crate::ValidatedFixedPrecoloredSplitRequirements,
    physical: &ValidatedPhysicalRegisterModel,
    policy: FixedPrecoloredSegmentHomePolicy,
    budget: OptimizationWorkBudget,
) -> Result<ReplayedHomes, FixedPrecoloredSegmentHomeError> {
    match policy {
        FixedPrecoloredSegmentHomePolicy::MostConstrainedLowestCompatibleViewV1 => {}
    }
    let mut work = work::Work::new();
    let functions = family(
        &ranges.plan().functions,
        &requirements.plan().functions,
        physical,
        &mut work,
    )?;
    let structural_unit_functions = family(
        &ranges.plan().structural_unit_functions,
        &requirements.plan().structural_unit_functions,
        physical,
        &mut work,
    )?;
    let usage = work.finish(budget)?;
    Ok(ReplayedHomes {
        functions,
        structural_unit_functions,
        usage,
    })
}

fn family(
    ranges: &[FunctionLiveRanges],
    requirements: &[FunctionFixedPrecoloredSplitRequirements],
    physical: &ValidatedPhysicalRegisterModel,
    work: &mut work::Work,
) -> Result<Vec<FunctionFixedPrecoloredSegmentHomes>, FixedPrecoloredSegmentHomeError> {
    if ranges.len() != requirements.len() {
        return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
    }
    let requirements = indexes::requirements(requirements)?;
    ranges
        .iter()
        .enumerate()
        .map(|(function, ranges)| {
            let source = requirements
                .get(&ranges.machine)
                .copied()
                .ok_or(FixedPrecoloredSegmentHomeError::FunctionMismatch { function })?;
            let domains = domains::reconstruct(function, source, work)?;
            let conflicts = conflicts::reconstruct(function, &domains, ranges, physical, work)?;
            placement::reconstruct(function, ranges.machine, &domains, &conflicts, work)
        })
        .collect()
}
