//! Array operands use the same checked pure values and computations as calls.
//! Each element retains a statement-local role and exact primitive destination;
//! selected constant projections still retain every indexing selection.

use super::*;
use typed_trees::expression::ExpressionHandle;

pub(super) fn elements(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    source: checked_trees::CheckedArrayConstructionSource,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
    let leaves = validation::scalar_array_elements(program, machine, expression, expected)?;
    for projection in leaves.projections {
        if let Some(selected) = facts.operators.expression_use(projection)
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return None;
        }
    }
    leaves
        .elements
        .into_iter()
        .enumerate()
        .map(|(element_index, (expression, primitive_type))| {
            let role = CheckedScalarExpressionRole::ArrayElement {
                source,
                element_ordinal: u32::try_from(element_index).ok()?,
            };
            let computations = &facts.values.scalar_computations;
            let roots = computations
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| {
                    root.state == state
                        && root.statement_ordinal == statement_ordinal
                        && root.role == role
                })
                .collect::<Vec<_>>();
            if let [root] = roots.as_slice() {
                if root.machine != machine
                    || !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).authored_root != expression
                    || computations.nodes.get(root.root).primitive_type != primitive_type
                    || facts
                        .values
                        .scalar_expressions
                        .expressions
                        .iter()
                        .any(|value| {
                            value.state == state
                                && value.statement_ordinal == statement_ordinal
                                && value.role == role
                        })
                {
                    return None;
                }
                return Some(checked_trees::CheckedCallScalarArgument::Computation(
                    root.root,
                ));
            }
            if !roots.is_empty() {
                return None;
            }
            let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
                state,
                statement_ordinal,
                role,
            )?;
            if binding.expression != expression
                || crate::values::scalar_expression_type(value) != Some(primitive_type)
            {
                return None;
            }
            Some(checked_trees::CheckedCallScalarArgument::Pure(
                value.clone(),
            ))
        })
        .collect()
}
