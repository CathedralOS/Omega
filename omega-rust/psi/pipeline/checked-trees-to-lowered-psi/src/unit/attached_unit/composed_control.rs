//! Ordinary state graphs share one callable closure and publication path.
use super::{CheckedTrees, LoweringError};
mod admission;
pub(super) mod callable;
mod catalogs;
pub(super) mod dynamic_result;
mod emission;
mod internal_calls;
mod literal_arguments;
mod scalar_calls;
mod state_graph;
pub(crate) use crate::producer_result::SourceMappedLowered;
pub(super) use callable::admit as admit_callable;
pub(crate) use catalogs::ComposedCatalogs;

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    if !state_graph::has_shared_graph_custody(checked, plan) {
        return super::unsupported("composed Unit control has unsupported graph custody");
    }
    let mut lowered = super::lower_unit_effect_closure(checked, plan.machine)?;
    super::finalize_operation_proofs(&mut lowered.terminal)?;
    Ok(lowered)
}
