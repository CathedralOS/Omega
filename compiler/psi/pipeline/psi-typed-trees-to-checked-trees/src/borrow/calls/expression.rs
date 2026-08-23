use super::*;
use crate::lookup::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
};

pub(super) fn collect_expression_borrow_calls(
    collection: &mut BorrowCallCollection<'_>,
    expression: ExpressionHandle,
) {
    match collection.program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_expression_borrow_calls(collection, atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in collection
                .program
                .expression_table
                .expression_handles(*values)
            {
                collect_expression_borrow_calls(collection, *value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                collect_expression_borrow_calls(collection, range.start);
            }
            if range.end.is_valid() {
                collect_expression_borrow_calls(collection, range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_borrow_calls(collection, binary.left);
            collect_expression_borrow_calls(collection, binary.right);
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(collection.program, call.receiver);
            let resolved_target = resolve_state_call_target(
                collection.program,
                collection.machine,
                collection.state,
                receiver_symbol,
                call.target_symbol,
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
                );
                collection.append_borrow_call(
                    receiver_symbol,
                    if resolved_target.is_valid() {
                        resolved_target
                    } else {
                        call.target_symbol
                    },
                    receiver_path.is_some(),
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
        ExpressionNode::Cast(cast) => collect_expression_borrow_calls(collection, cast.value),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(collection, indexed.collection);
            collect_expression_borrow_calls(collection, indexed.index);
        }
        ExpressionNode::Member(member) => {
            collect_expression_borrow_calls(collection, member.receiver)
        }
        ExpressionNode::Mutable(inner_expression) => {
            collect_expression_borrow_calls(collection, *inner_expression)
        }
        ExpressionNode::Unary(unary) => collect_expression_borrow_calls(collection, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in collection
                .program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_borrow_calls(collection, field.value);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
