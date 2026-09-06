//! Dynamic-result continuation admission and catalog lowering.

use super::*;

pub(crate) use super::emission::emit_call_leaf;

pub(crate) fn lower_control_catalogs(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &checked_trees::CheckedDynamicUnitContinuationPlan,
    stored: Option<&checked_trees::CheckedStoredDynamicScalarCallPlan>,
) -> Result<ComposedCatalogs, LoweringError> {
    let (boundaries, internal_targets) =
        admission::admit_dynamic_continuation(checked, plan, continuation, stored)?;
    catalogs::lower_dynamic_catalogs(checked, plan, continuation, &boundaries, &internal_targets)
}
