//! Array operands use the same checked pure values and computations as calls.
//! Each element retains a statement-local role and exact primitive destination;
//! selected constant projections still retain every indexing selection.
use super::{CheckFacts, CheckedScalarExpressionRole, SymbolHandle, TypeReferenceHandle};
use typed_trees::expression::ExpressionHandle;

pub(in crate::execution::terminal_unit) fn elements(
    program: &dyn ::validation::CheckingInput,
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
            && !selected.resolved_through_builtin_fallback(language_core::OperatorSpelling::Index)
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
                if !computations.is_authored_root(root, machine, expression, primitive_type)
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

/// A whole closed primitive array assignment is one value establishment before
/// any receiving storage changes. Keep selected operators and qualifications
/// at their existing owners; only builtin literal array construction lands here.
pub(in crate::execution::terminal_unit) fn assignment_type(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    assignment: &typed_trees::statement::TableAssignment,
) -> Option<TypeReferenceHandle> {
    if !matches!(
        program.expression_table.expression(assignment.value),
        typed_trees::expression::ExpressionNode::ArrayLiteral(_)
    ) || !validation::place_has_builtin_coordinates(
        program,
        machine,
        Some(state),
        assignment.target,
    ) {
        return None;
    }
    let expected = validation::closed_array_store_type(
        program,
        validation::assignment_stored_value_type(program, machine, state, assignment)?,
    )?;
    // Packing is optional. Nested memberships require the ordinary completed
    // array transaction, including for whole write-only replacements.
    validation::scalar_array_preserves_memberships(program, expected).then_some(expected)
}
