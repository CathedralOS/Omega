//! Exact route selection for checked composed Unit control.

use super::super::super::CheckedComposedUnitControlTerminatorPlan;
use super::super::finalize_operation_proofs;
use super::{CheckedTrees, LoweringError, SourceMappedLowered, closed_sum, state_graph};
pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    if state_graph::has_shared_graph_custody(checked, plan) {
        let mut lowered =
            crate::unit::attached_unit::lower_unit_effect_closure(checked, plan.machine)?;
        finalize_operation_proofs(&mut lowered.terminal)?;
        return Ok(lowered);
    }
    if matches!(
        plan.states.first().map(|state| &state.terminator),
        Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. })
    ) {
        return closed_sum::lower(checked, plan);
    }
    super::super::unsupported("composed Unit control has unsupported graph custody")
}
