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
        // A call appearing in a *value* sub-expression position (a nested
        // operator/boundary or state call used as an aggregate element/field, a
        // binary or range operand, or a cast operand) still transfers ownership
        // of any owned by-value arguments it consumes. Recursion reaches such a
        // call only *through* an enclosing value expression; the call-flow
        // discovery pass only records argument moves for state borrow calls, so
        // for non-state (operator/boundary) calls reached this way the owned
        // argument transfers would otherwise leave no ownership event. Descend
        // into the call's owned by-value arguments here.
        //
        // State borrow calls (`find_state(target) == Some`) are intentionally
        // excluded: their by-value argument transfers are emitted by
        // `append_call_ownership_events` from the discovered `BorrowCallFact`s,
        // so descending into them here would double-count.
        ExpressionNode::Call(call) => {
            append_move_events_for_call_arguments(
                program,
                ctx,
                state_symbol,
                statement_index,
                call,
                source,
            );
        }
        ExpressionNode::Mutable(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

/// Append type-aware move events for the owned by-value arguments (and receiver)
/// of a call that is itself reached as a value sub-expression.
///
/// State borrow calls are skipped because the call-flow pass already records
/// their argument transfers from the discovered borrow-call facts. For an
/// operator/boundary call the per-parameter ownership policy comes from the
/// callee's declared parameter types (the same source of truth as
/// [`classify_operator_result_ownership`]); a parameter whose type copies reads
/// its argument by value and transfers nothing, so it produces no move event.
/// When no operator declaration is found the argument expression's own
/// type-aware policy decides via the recursive descent.
fn append_move_events_for_call_arguments(
    program: &omega_typed_trees::TypedTrees,
    ctx: &mut FlowBuildContext,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call: &omega_typed_trees::expression::TableCallExpression,
    source: FlowOwnershipEventSource,
) {
    // State borrow calls are owned by the call-flow argument-move pass.
    if find_state(program, call.target_symbol).is_some() {
        return;
    }

    if call.receiver.is_valid() {
        append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            call.receiver,
            source,
        );
    }

    let arguments = program.expression_table.expression_handles(call.arguments);
    let parameter_ownership = call_parameter_ownership(program, call.target_symbol);

    for (ordinal, argument) in arguments.iter().enumerate() {
        // When the callee's parameter type is known and copies (does not own),
        // the argument is read by value without transferring ownership.
        if let Some(false) = parameter_ownership
            .as_ref()
            .and_then(|owns| owns.get(ordinal).copied())
        {
            continue;
        }

        append_move_events_for_expression(
            program,
            ctx,
            state_symbol,
            statement_index,
            *argument,
            source,
        );
    }
}

/// The per-parameter ownership policy of a call's resolved operator/boundary
/// definition, if one is found. The returned vector excludes the implicit `self`
/// receiver parameter so its entries line up with the call's positional
/// arguments. `None` means the callee is not a known operator definition, so the
/// argument expression's own type should decide.
fn call_parameter_ownership(
    program: &omega_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<Vec<bool>> {
    let operator = operator_definition_for_call(program, target_symbol)?;
    Some(
        program
            .operator_parameters(operator)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| type_requires_ownership(program, parameter.type_reference))
            .collect(),
    )
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
