//! General state traversal for claim-free Unit graphs in the shared call catalog.

use super::*;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
    CheckedStructuralControlSuccessorPlan,
};

mod admission;
mod edges;
mod emission;
mod scalars;
mod subslices;
mod topology;

pub(in crate::attached_unit) use admission::AdmittedGraph;
pub(super) use admission::admit;
pub(super) use admission::has_shared_graph_custody;
use edges::successors;
pub(super) use emission::emit;
