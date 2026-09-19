//! Exact composed Unit control routed by checked topology.
use super::{CheckedTrees, LoweringError};
mod admission;
pub(super) mod callable;
mod catalogs;
mod closed_sum;
pub(super) mod dynamic_result;
mod emission;
mod internal_calls;
mod literal_arguments;
mod routing;
mod scalar_calls;
mod state_graph;
pub(crate) use crate::producer_result::SourceMappedLowered;
pub(super) use callable::admit as admit_callable;
pub(crate) use catalogs::ComposedCatalogs;

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    routing::lower(checked, plan)
}
