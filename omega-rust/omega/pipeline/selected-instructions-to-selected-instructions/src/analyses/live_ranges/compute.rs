//! Live-range plan computation and its semantic derivation families.

mod architectural_units;
mod constraints;
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
use function::{compute_function, compute_structural_function};

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
    let structural_unit_functions = selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .zip(&liveness.plan().structural_unit_functions)
        .enumerate()
        .map(|(index, (selected, live))| compute_structural_function(index, selected.machine, live))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LiveRangePlan {
        selected: selected.selected_identity(),
        liveness: liveness.receipt().identity(),
        optimization_unit: selected.optimization_unit_identity(),
        fuel_schedule: selected.fuel_schedule_identity(),
        target: selected.selected_plan().target,
        functions,
        structural_unit_functions,
    })
}

#[cfg(test)]
mod tests;
