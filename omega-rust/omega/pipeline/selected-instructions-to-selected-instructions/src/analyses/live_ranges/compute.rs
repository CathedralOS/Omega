//! Live-range plan computation and its semantic derivation families.

mod architectural_units;
mod constraints;
mod edge_transfers;
mod fragments;
mod function;

use std::collections::BTreeSet;

use crate::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockLiveness, BlockPointDomain, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse,
    FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeError, LiveRangeFragment, LiveRangePlan,
    LiveRangePoint, LivenessPosition, ValidatedLiveness, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
};
use register_model::{RegisterOperandAccess, RegisterUnitId};
use selected_instructions::{SelectedBlockId, VirtualRegisterId};

use architectural_units::architectural_units;
#[cfg(test)]
use architectural_units::build_unit;
pub(crate) use constraints::{derive_early_clobbers, derive_tied_pairs};
use fragments::{
    after_point, before_point, block_domain, connector, fragments_from_points, fragments_overlap,
    operand_point, virtual_fragments,
};
use function::compute_function;

#[cfg(test)]
std::thread_local! {
    pub(crate) static REUSED_FUNCTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn compute_terminal_live_ranges(
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<LiveRangePlan, LiveRangeError> {
    let functions = selected
        .selected_plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .enumerate()
        .map(|(index, (selected, live))| compute_function(index, selected, live))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LiveRangePlan {
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: selected.selected_plan().target,
        functions,
    })
}

#[cfg(test)]
mod tests;

pub(crate) fn compute_terminal_live_ranges_reusing(
    previous: &impl crate::ValidatedSelectedAnalysis,
    previous_liveness: &ValidatedLiveness,
    previous_ranges: &crate::ValidatedLiveRanges,
    selected: &impl crate::ValidatedSelectedAnalysis,
    liveness: &ValidatedLiveness,
) -> Result<LiveRangePlan, LiveRangeError> {
    let prior = previous_ranges.plan();
    let compatible = prior.selected == previous.selected_identity()
        && prior.liveness == previous_liveness.receipt().identity()
        && previous_liveness.plan().selected == previous.selected_identity()
        && prior.optimization_unit == previous.optimization_unit_identity()
        && prior.fuel_schedule == previous.fuel_schedule_identity()
        && prior.target == previous.selected_plan().target
        && prior.optimization_unit == selected.optimization_unit_identity()
        && prior.fuel_schedule == selected.fuel_schedule_identity()
        && prior.target == selected.selected_plan().target
        && prior.functions.len() == previous.selected_plan().functions.len();
    let functions = selected
        .selected_plan()
        .functions
        .iter()
        .zip(&liveness.plan().functions)
        .enumerate()
        .map(|(function_index, (function, live))| {
            if compatible
                && (previous
                    .selected_plan()
                    .functions
                    .shares_function_storage(&selected.selected_plan().functions, function_index)
                    || previous.selected_plan().functions.get(function_index) == Some(function))
                && previous_liveness.plan().functions.get(function_index) == Some(live)
                && let Some(facts) = prior.functions.get(function_index)
            {
                #[cfg(test)]
                REUSED_FUNCTIONS.set(REUSED_FUNCTIONS.get() + 1);
                return Ok(facts.clone());
            }
            compute_function(function_index, function, live)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LiveRangePlan {
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: selected.selected_plan().target,
        functions,
    })
}
