//! A state's scalar successor values, in the shared
//! source-bound value namespace.
use super::super::super::{CheckedScalarExpressionRole, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::CheckedComposedUnitControlStatePlan;

/// Select the retained value at its authored successor coordinate. Storage
/// reads use computation nodes; pure expressions keep their ordinary binding.
pub(in crate::unit::attached_unit::composed_control) fn successor_value(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &super::CheckedStructuralControlSuccessorPlan,
    argument: &checked_trees::CheckedStructuralScalarArgumentPlan,
) -> Result<checked_trees::CheckedCallScalarArgument, LoweringError> {
    let role = CheckedScalarExpressionRole::TransitionArgument {
        argument_ordinal: argument.argument_ordinal,
    };
    let pure = &checked.facts.values.scalar_expressions;
    let computations = &checked.facts.values.scalar_computations;
    let mut roots = computations
        .roots
        .iter()
        .map(|(_, root)| root)
        .filter(|root| {
            root.state == state.state
                && root.statement_ordinal == edge.statement_ordinal
                && root.role == role
        });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("Unit graph successor computation has duplicate source roots");
    }
    if let Some(root) = root {
        // Failed uniqueness is not absence: an incomplete or duplicated pure
        // roster must not disappear behind the computation lane.
        let has_pure = pure.source_bindings.iter().any(|(_, binding)| {
            binding.state == state.state
                && binding.statement_ordinal == edge.statement_ordinal
                && binding.role == role
        }) || pure.expressions.iter().any(|expression| {
            expression.state == state.state
                && expression.statement_ordinal == edge.statement_ordinal
                && expression.role == role
        });
        if has_pure
            || !computations.nodes.is_valid(root.root)
            || computations.nodes.get(root.root).primitive_type != argument.primitive_type
        {
            return unsupported("Unit graph successor computation disagrees with its argument");
        }
        Ok(checked_trees::CheckedCallScalarArgument::Computation(
            root.root,
        ))
    } else {
        let (_, value) = pure
            .bound_expression_at(state.state, edge.statement_ordinal, role)
            .ok_or(LoweringError::Unsupported(
                "Unit graph scalar successor has no checked source expression",
            ))?;
        Ok(checked_trees::CheckedCallScalarArgument::Pure(
            value.clone(),
        ))
    }
}
