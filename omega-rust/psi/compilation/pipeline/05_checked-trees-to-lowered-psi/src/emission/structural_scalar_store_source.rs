//! The computation root that supplies a projected scalar store's value.
use super::{CheckedScalarExpressionRole, CheckedTrees, LoweringError, unsupported};

/// Select a computation only through its exact authored assignment coordinate.
/// A whole-record replacement selects each field through the literal's own
/// `RecordField` root rather than the assignment's role, so the returned role
/// is the one the resolved root actually carries.
pub(crate) fn computation_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedStoreValue,
    field_identity: Option<&str>,
) -> Result<
    Option<(
        checked_trees::CheckedScalarComputationHandle,
        CheckedScalarExpressionRole,
    )>,
    LoweringError,
> {
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement_index && root.role == role
    });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("field assignment has duplicate computation roots");
    }
    let checked_trees::CheckedStoreValue::Computation(handle) = value else {
        return if root.is_none() {
            Ok(None)
        } else {
            unsupported("field assignment replaced its computation with a pure value")
        };
    };
    let handle = *handle;
    let root = match root {
        Some(root) => root,
        None => {
            let mut record_roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == state
                    && root.statement_ordinal == statement_index
                    && root.root == handle
                    && matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
            });
            let root = record_roots.next().ok_or(LoweringError::Unsupported(
                "field assignment has no computation root",
            ))?;
            if record_roots.next().is_some() {
                return unsupported("field assignment has duplicate record computation roots");
            }
            root
        }
    };
    if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
        return unsupported("field assignment computation root has different custody");
    }
    if matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
        && field_identity.is_none()
    {
        return unsupported("record store field has no field identity");
    }
    Ok(Some((handle, root.role)))
}
