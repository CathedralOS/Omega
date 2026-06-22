use omega_typed_trees::expression::ExpressionHandle;

use super::super::super::facts::RangeFacts;

pub(in crate::checks::ranges::guards) fn seed_at_most_fact(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    lower: ExpressionHandle,
    upper: ExpressionHandle,
) {
    facts.prove_at_most(
        program.expression_table.display_name(lower),
        program.expression_table.display_name(upper),
    );
}
