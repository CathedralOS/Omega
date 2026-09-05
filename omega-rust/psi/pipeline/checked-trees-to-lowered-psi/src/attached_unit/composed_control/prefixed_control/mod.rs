//! Exact scalar-prefix composition before one conditional effect frontier.

use super::*;

mod admission;
mod emission;

pub(super) fn lower(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<LoweredPsi, LoweringError> {
    let admitted = admission::admit(checked, plan)?;
    let catalogs = super::catalogs::lower_composed_catalogs(checked, plan, &admitted.leaf_calls)?;
    emission::emit(checked, plan, admitted, catalogs)
}
