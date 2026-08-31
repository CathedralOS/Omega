//! Exact composed Unit control routed by checked topology.
use super::*;

mod admission;
mod catalogs;
mod custody;
mod emission;
mod internal_calls;
mod nested_control;
mod prefixed_control;

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
