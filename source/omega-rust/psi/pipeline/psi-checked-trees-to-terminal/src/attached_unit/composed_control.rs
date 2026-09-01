//! Exact composed Unit control routed by checked topology.
use super::*;

mod admission;
mod catalogs;
mod custody;
mod emission;
mod internal_calls;
mod nested_control;
mod prefixed_control;

pub(crate) use catalogs::ComposedCatalogs;
pub(crate) use emission::emit_boundary_leaf as emit_direct_dynamic_boundary_leaf;

pub(crate) fn lower_direct_dynamic_control_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedDirectDynamicScalarCallPlan,
    continuation: &psi_checked_trees::CheckedDirectDynamicUnitContinuationPlan,
) -> Result<ComposedCatalogs, LoweringError> {
    let boundaries = admission::admit_direct_dynamic_continuation(checked, plan, continuation)?;
    catalogs::lower_direct_dynamic_catalogs(checked, plan, continuation, &boundaries)
}

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if plan.states.len() >= 4
        && matches!(
            plan.states[0].terminator,
            CheckedComposedUnitControlTerminatorPlan::Jump { .. }
        )
    {
        return prefixed_control::lower(checked, plan);
    }
    if plan.states.len() >= 4 {
        return nested_control::lower(checked, plan);
    }
    let admitted = admission::admit_composed_unit_control(checked, plan)?;
    let catalogs = catalogs::lower_composed_catalogs(checked, plan, &admitted)?;
    emission::emit_composed_unit_control(checked, plan, admitted, catalogs)
}
