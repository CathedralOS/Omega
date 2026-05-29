use super::*;

pub(super) fn append_move_events_for_expression(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) {
    if type_resolution::expression_requires_ownership(
        program,
        state_symbol,
        statement_index,
        expression,
    ) {
        if let Some(place) = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) {
            append_move_event_for_place(ctx, place, source);
        }
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    *value,
                    source,
                );
            }
        }
        ExpressionNode::Cast(cast) => append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            cast.value,
            source,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    field.value,
                    source,
                );
            }
        }
        ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_) => {}
    }
}
