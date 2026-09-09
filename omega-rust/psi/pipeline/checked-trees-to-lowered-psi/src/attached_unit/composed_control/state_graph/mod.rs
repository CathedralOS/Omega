//! General state traversal for claim-free Unit graphs in the shared call catalog.

use super::*;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan,
};

mod admission;
pub(super) mod body;
mod case_emission;
mod cases;
mod edges;
mod emission;
mod parameters;
mod ranking;
mod scalars;
mod subslices;
#[cfg(test)]
mod tests;
mod topology;

pub(in crate::attached_unit) use admission::AdmittedGraph;
pub(super) use admission::admit;
pub(super) use admission::has_shared_graph_custody;
use edges::successors;
pub(super) use emission::emit;
