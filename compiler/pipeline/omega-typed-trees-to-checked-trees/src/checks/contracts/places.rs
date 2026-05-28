use crate::labels::canonical_place_label;

pub(super) fn expression_is_boolean_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            expression_is_boolean_place_like(program, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Name(_)
        | omega_typed_trees::expression::ExpressionNode::Member(_)
        | omega_typed_trees::expression::ExpressionNode::Indexed(_) => true,
        _ => false,
    }
}

pub(super) fn expression_place_matches(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    expression: omega_typed_trees::expression::ExpressionHandle,
    candidate_place: omega_facts::PlaceHandle,
) -> bool {
    let candidate_label =
        canonical_place_label(program, semantic, semantic.places.get(candidate_place));
    program.expression_table.display_name(expression) == candidate_label
}
