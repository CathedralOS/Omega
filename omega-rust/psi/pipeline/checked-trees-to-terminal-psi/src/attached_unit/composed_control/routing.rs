//! Exact route selection for checked composed Unit control.

use super::*;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    if matches!(
        plan.states.first().map(|state| &state.terminator),
        Some(CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. })
    ) {
        return closed_sum::lower(checked, plan);
    }
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
