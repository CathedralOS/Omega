pub(super) fn expression_is_boolean_place_like(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Borrow(
            inner,
        ) => expression_is_boolean_place_like(program, inner.target),
        symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Name(_)
        | symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Member(
            _,
        )
        | symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode::Indexed(
            _,
        ) => true,
        _ => false,
    }
}

pub(super) fn expression_place_matches(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &crate::fact_plan::FactPlan,
    expression: symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionHandle,
    candidate_place: crate::fact_plan::PlaceHandle,
) -> bool {
    let candidate_label = semantic.place_label(program, candidate_place);
    program.expression_table.display_name(expression) == candidate_label
}
