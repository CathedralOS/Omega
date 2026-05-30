use super::type_references::{OperatorResultOwnership, classify_operator_result_ownership};
use super::*;

pub(super) fn append_move_events_for_expression(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    source: FlowOwnershipEventSource,
) {
    if type_references::expression_requires_ownership(
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
        ExpressionNode::Binary(binary) => {
            append_move_events_for_expression(
                program,
                ctx,
                state_symbol,
                statement_index,
                binary.left,
                source,
            );
            append_move_events_for_expression(
                program,
                ctx,
                state_symbol,
                statement_index,
                binary.right,
                source,
            );
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
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    range.start,
                    source,
                );
            }
            if range.end.is_valid() {
                append_move_events_for_expression(
                    program,
                    ctx,
                    state_symbol,
                    statement_index,
                    range.end,
                    source,
                );
            }
        }
        ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Whether a `let`-initializer expression produces a freshly owned value through
/// a boundary/slice/string/collection operator call (as opposed to borrowing a
/// view or reading an existing place).
///
/// Used to decide whether an owned binding's initializer warrants a move event
/// into the bound local. Operator results are classified through
/// [`classify_operator_result_ownership`], keeping operator-result ownership
/// policy in one place; a place-like initializer is excluded because its
/// transfer is already recorded by the source-side move.
pub(in crate::flow::ownership) fn initializer_produces_owned_value(
    program: &omega_typed_trees::TypedTrees,
    initializer: ExpressionHandle,
) -> bool {
    if !initializer.is_valid() {
        return false;
    }

    let target_symbol = match program.expression_table.expression(initializer) {
        ExpressionNode::Call(call) => call.target_symbol,
        _ => return false,
    };

    operator_definition_for_call(program, target_symbol)
        .map(|operator| {
            classify_operator_result_ownership(program, operator.return_type)
                == OperatorResultOwnership::OwnedValue
        })
        .unwrap_or(false)
}

/// Resolve the boundary/operator definition a call dispatches to, if any.
fn operator_definition_for_call(
    program: &omega_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&omega_typed_trees::operator::OperatorDefinition> {
    if !target_symbol.is_valid() {
        return None;
    }
    program
        .operators()
        .iter()
        .find(|operator| operator.symbol == target_symbol)
}
