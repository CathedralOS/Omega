//! Dynamic-result continuation admission and catalog lowering.

use super::*;

pub(crate) use super::emission::emit_boundary_leaf;

pub(crate) fn lower_control_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &psi_checked_trees::CheckedDynamicUnitContinuationPlan,
    stored: Option<&psi_checked_trees::CheckedStoredDynamicScalarCallPlan>,
) -> Result<ComposedCatalogs, LoweringError> {
    let boundaries = admission::admit_dynamic_continuation(checked, plan, continuation, stored)?;
    catalogs::lower_dynamic_catalogs(checked, plan, continuation, &boundaries)
}
