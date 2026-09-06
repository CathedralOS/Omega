//! Rejoin a computation root through the shared authored scalar locator.

use super::*;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    site: &Site<'_>,
    role: CheckedScalarExpressionRole,
    root: Computation,
    expected_destination: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root) {
        return unsupported("scalar computation has no live root");
    }
    let source = super::super::source_custody::locate(checked, site.state, site.statement, role)?;
    let destination = if matches!(
        role,
        CheckedScalarExpressionRole::StorageInitializer
            | CheckedScalarExpressionRole::AssignmentValue
    ) {
        source.destination
    } else {
        symbols::SymbolHandle::invalid()
    };
    let root = plans.nodes.get(root);
    if source.machine != machine
        || source.expression != root.authored_root
        || source.primitive_type != root.primitive_type
        || destination != expected_destination
    {
        return unsupported("scalar computation root disagrees with its authored destination");
    }
    Ok(())
}
