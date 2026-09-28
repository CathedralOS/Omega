//! Primitive locals at authored statement and dense binding positions.

use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitScalarResultBindingPlan, TypedTrees,
};
pub(super) fn scalar_expression_local_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)> {
    if local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
    {
        return None;
    }
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    let value = facts.values.scalar_expressions.expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
    )?;
    if crate::values::scalar_expression_type(value) != Some(primitive_type) {
        return None;
    }
    Some((
        CheckedUnitScalarResultBindingPlan {
            statement_index,
            binding_ordinal,
            primitive_type,
        },
        value.clone(),
    ))
}
