use crate::borrow::calls::BorrowCallCollection;
use validation::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
};

use checked_trees::expression::{ExpressionHandle, ExpressionNode};

pub(super) fn collect_expression_borrow_calls(
    collection: &mut BorrowCallCollection<'_>,
    expression: ExpressionHandle,
) {
    // Normalize one expression occurrence once, even when several DAG edges
    // use its value. Borrow and semantic lookup must assign the same call
    // ordinal; distinct authored handles still establish distinct loans.
    if collection.expressions.contains(&expression) {
        return;
    }
    collection.expressions.push(expression);
    let program = collection.program;
    if super::super::children::recurse_shared_children(program, expression, &mut |child| {
        collect_expression_borrow_calls(collection, child);
    }) {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(collection.program, call.receiver);
            let selected_operator =
                typed_trees::operator::resolve_named_expression_call(collection.program, call);
            let selected_target = validation::call_target_symbol(program, call);
            let resolved_target = resolve_state_call_target(
                collection.program,
                collection.machine,
                collection.state,
                receiver_symbol,
                selected_target,
                receiver_path.as_deref(),
                &call.target,
            );
            let is_machine_call = resolved_target.is_valid()
                || receiver_can_dispatch_to_machine(
                    collection.program,
                    collection.machine,
                    collection.state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            if is_machine_call {
                let accesses = collection.collect_call_argument_accesses(
                    collection
                        .program
                        .expression_table
                        .expression_handles(call.arguments),
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                );
                collection.append_borrow_call(
                    expression,
                    receiver_symbol,
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                    selected_operator.map_or_else(
                        || {
                            call.receiver.is_valid()
                                && collection.program.symbols.get(receiver_symbol).kind
                                    != symbols::SymbolKind::ConformanceParameter
                        },
                        |operator| {
                            // A qualified requirement namespace is not a
                            // runtime operand. Only the declaration's extra
                            // receiver position establishes receiver custody.
                            call.receiver.is_valid()
                                && collection.program.operator_parameters(operator).len()
                                    == collection
                                        .program
                                        .expression_table
                                        .expression_handles(call.arguments)
                                        .len()
                                        + 1
                        },
                    ),
                    accesses,
                );
            }

            if call.receiver.is_valid() {
                collect_expression_borrow_calls(collection, call.receiver);
            }
            for argument in collection
                .program
                .expression_table
                .expression_handles(call.arguments)
            {
                collect_expression_borrow_calls(collection, *argument);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(collection, indexed.collection);
            collect_expression_borrow_calls(collection, indexed.index);
        }
        ExpressionNode::Member(member) => {
            collect_expression_borrow_calls(collection, member.receiver)
        }
        ExpressionNode::Borrow(inner_expression) => {
            collect_expression_borrow_calls(collection, inner_expression.target)
        }
        _ => {}
    }
}
