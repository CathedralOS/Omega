//! General acyclic Boolean control graphs with exact effect leaves.
use super::super::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedComposedUnitControlMachinePlan, TypedTrees,
};

use crate::execution::terminal_unit::ShapeCollector;

#[path = "assembly.rs"]
mod assembly;
#[path = "operations.rs"]
mod operations;
#[path = "topology.rs"]
mod topology;

pub(super) fn build(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedComposedUnitControlMachinePlan> {
    let topology = topology::admit(program, facts, shapes, machine)?;
    assembly::finish(program, facts, shapes, boundaries, machine, topology)
}
