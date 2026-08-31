//! Atomic multi-state Unit control plans.

use super::*;

mod assembly;
mod guards;
mod leaves;
mod topology;

pub(super) fn build_checked_composed_unit_control_machines(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> Vec<CheckedComposedUnitControlMachinePlan> {
    program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| assembly::build(program, facts, shapes, boundaries, machine))
        .collect()
}
