//! General acyclic Boolean control graphs with exact effect leaves.

use super::*;

mod assembly;
mod operations;
mod topology;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &psi_typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let topology = topology::admit(program, facts, shapes, machine)?;
    assembly::finish(program, facts, shapes, boundaries, machine, topology)
}
