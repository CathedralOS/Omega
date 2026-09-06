//! Exact composed Unit control routed by checked topology.
use super::*;
mod admission;
mod catalogs;
mod closed_sum;
mod custody;
pub(super) mod dynamic_result;
mod emission;
mod internal_calls;
mod nested_control;
mod prefixed_control;
mod routing;
mod scalar_calls;
pub(crate) use catalogs::ComposedCatalogs;
pub(crate) struct ComposedLowered {
    pub(crate) terminal: LoweredPsi,
    /// Exact catalog owners, ordered by the emitted machine table.
    pub(crate) source_machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
}

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<ComposedLowered, LoweringError> {
    routing::lower(checked, plan)
}
