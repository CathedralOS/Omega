use crate::labels::canonical_place_label;

pub(super) fn expression_is_boolean_place_like(
    program: &psi_typed_trees::TypedTrees,
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        psi_typed_trees::expression::ExpressionNode::Borrow(inner) => {
            expression_is_boolean_place_like(program, inner.target)
        }
        psi_typed_trees::expression::ExpressionNode::Name(_)
        | psi_typed_trees::expression::ExpressionNode::Member(_)
        | psi_typed_trees::expression::ExpressionNode::Indexed(_) => true,
        _ => false,
    }
}

pub(super) fn expression_place_matches(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    expression: psi_typed_trees::expression::ExpressionHandle,
    candidate_place: psi_facts::PlaceHandle,
) -> bool {
    let candidate_label =
        canonical_place_label(program, semantic, semantic.places.get(candidate_place));
    program.expression_table.display_name(expression) == candidate_label
}
