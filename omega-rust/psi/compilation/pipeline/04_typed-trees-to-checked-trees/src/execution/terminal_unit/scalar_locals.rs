//! Primitive locals at authored statement and dense binding positions.

use super::{
    CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedUnitScalarResultBindingPlan,
};
pub(super) fn scalar_expression_local_at(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    binding_ordinal: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)> {
    if local.is_mutable {
        return None;
    }
    let primitive_type = program.primitive_type_reference(local.type_reference)?;
    // The value-only state prefix and this ordinary statement sequence share
    // one custody contract: the plan row must bind this local's own
    // initializer, not merely sit at its coordinate. A missing authored
    // expression is legitimate only through the scalar producer's checked
    // zero initializer for this exact declaration; absence alone grants no value.
    let (custody, value) = facts.values.scalar_expressions.bound_expression_at(
        state.symbol,
        statement_index,
        CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
    )?;
    if custody.expression != local.initial_value
        || custody.destination != local.symbol
        || crate::values::scalar_expression_type(value) != Some(primitive_type)
    {
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
