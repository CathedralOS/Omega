//! Exact three-block Unit control with one boundary effect in each leaf.
//!
//! Admission rejoins checked identities, catalogs publish the selected semantic
//! closure, and emission builds the three-block Terminal machine.

use super::*;

mod admission;
mod catalogs;
mod emission;

use admission::admit_composed_unit_control;
use catalogs::lower_composed_catalogs;
use emission::emit_composed_unit_control;

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let admitted = admit_composed_unit_control(checked, plan)?;
    let catalogs = lower_composed_catalogs(checked, plan, &admitted)?;
    emit_composed_unit_control(checked, plan, admitted, catalogs)
}
