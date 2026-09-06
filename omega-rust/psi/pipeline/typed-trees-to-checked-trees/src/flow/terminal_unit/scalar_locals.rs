//! Branch-free primitive locals following one result-bearing Unit operation.

use super::*;

pub(super) fn scalar_expression_local_suffix(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
) -> Option<Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>> {
    scalar_expression_locals_from(program, facts, state, statements, 1)
}

pub(super) fn scalar_expression_local_prefix(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
) -> Option<Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>> {
    scalar_expression_locals_from(program, facts, state, statements, 0)
}

fn scalar_expression_locals_from(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    start: usize,
) -> Option<Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>> {
    let local_count = statements
        .iter()
        .take_while(|statement| matches!(statement, StatementNode::LocalData(_)))
        .count();
    statements
        .get(start..local_count)?
        .iter()
        .enumerate()
        .map(|(suffix_index, statement)| {
            let statement_index = suffix_index.checked_add(start)?;
            let binding_ordinal = u32::try_from(statement_index).ok()?;
            let StatementNode::LocalData(local) = statement else {
                unreachable!("scalar-local suffix contains only local declarations")
            };
            if local.is_mutable || !local.initial_value.is_valid() {
                return None;
            }
            let primitive_type = program.primitive_type_reference(local.type_reference)?;
            let value = facts.values.scalar_expressions.expression_at(
                state.symbol,
                binding_ordinal,
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
            )?;
            if crate::values::scalar_expression_type(value) != Some(primitive_type)
                || matches!(
                    value,
                    CheckedScalarExpression::Boolean(expression)
                        if checked_boolean_contains_short_circuit(expression)
                )
            {
                return None;
            }
            Some((
                CheckedUnitScalarResultBindingPlan {
                    statement_index: binding_ordinal,
                    binding_ordinal,
                    primitive_type,
                },
                value.clone(),
            ))
        })
        .collect()
}
