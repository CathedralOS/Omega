//! Independent replay of general acyclic Boolean control graphs.

use super::*;

mod admission;
mod emission;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<LoweredTerminalPsi, LoweringError> {
    let admitted = admission::admit(checked, plan)?;
    let catalogs = super::catalogs::lower_composed_catalogs(checked, plan, &admitted.leaf_calls)?;
    emission::emit(checked, plan, admitted, catalogs)
}
