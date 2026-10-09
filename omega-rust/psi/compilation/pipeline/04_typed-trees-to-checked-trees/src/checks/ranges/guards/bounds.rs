mod indexes;
mod lengths;
mod orderings;

/// The member expression when `expression` is a `<collection>.length` read —
/// the bound shape every length-guard seeding rule gates on identically.
pub(super) fn length_member(
    program: &dyn ::validation::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<&typed_trees::expression::TableMemberExpression> {
    let member = program.expression_table.member_node(expression)?;
    (language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
        member.member.as_str(),
    ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
    .then_some(member)
}

pub(super) use indexes::{
    seed_at_most_len_range_bound_fact, seed_equal_len_offset_fact, seed_index_at_most_integer_fact,
    seed_index_less_than_integer_fact, seed_less_than_len_fact, seed_successor_at_most_len_fact,
};
pub(super) use lengths::{
    seed_length_at_least_fact, seed_length_equality_fact, seed_length_greater_than_fact,
    seed_length_not_zero_fact,
};
pub(super) use orderings::{seed_at_most_fact, seed_non_negative_fact};
