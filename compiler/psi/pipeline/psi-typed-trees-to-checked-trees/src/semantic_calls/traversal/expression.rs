use super::*;
use crate::lookup::{
    call_receiver_parts, receiver_can_dispatch_to_machine, resolve_state_call_target,
};

pub(super) fn find_call_site_in_expression<'program>(
    traversal: &mut CallSiteTraversal<'program, '_>,
    expression: ExpressionHandle,
) -> Option<CallSite<'program>> {
    match traversal.program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => find_call_site_in_expression(traversal, atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in traversal
                .program
                .expression_table
                .expression_handles(*values)
            {
                if let Some(call_site) = find_call_site_in_expression(traversal, *value) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Binary(binary) => find_call_site_in_expression(traversal, binary.left)
            .or_else(|| find_call_site_in_expression(traversal, binary.right)),
        ExpressionNode::Range(range) => {
            if range.start.is_valid()
                && let Some(call_site) = find_call_site_in_expression(traversal, range.start)
            {
                return Some(call_site);
            }
            range
                .end
                .is_valid()
                .then(|| find_call_site_in_expression(traversal, range.end))?
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(traversal.program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                traversal.program,
                traversal.machine,
                traversal.state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    traversal.program,
                    traversal.machine,
                    traversal.state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                )
                || call.target_symbol.is_valid();

            if is_machine_call {
                if traversal.is_target_call_site() {
                    return Some(CallSite::Expression { expression, call });
                }
                traversal.advance_call_ordinal();
            }

            if call.receiver.is_valid()
                && let Some(call_site) = find_call_site_in_expression(traversal, call.receiver)
            {
                return Some(call_site);
            }

            for argument in traversal
                .program
                .expression_table
                .expression_handles(call.arguments)
            {
                if let Some(call_site) = find_call_site_in_expression(traversal, *argument) {
                    return Some(call_site);
                }
            }

            None
        }
        ExpressionNode::Cast(cast) => find_call_site_in_expression(traversal, cast.value),
        ExpressionNode::Indexed(indexed) => {
            find_call_site_in_expression(traversal, indexed.collection)
                .or_else(|| find_call_site_in_expression(traversal, indexed.index))
        }
        ExpressionNode::Member(member) => find_call_site_in_expression(traversal, member.receiver),
        ExpressionNode::Borrow(inner) => find_call_site_in_expression(traversal, inner.target),
        ExpressionNode::Unary(unary) => find_call_site_in_expression(traversal, unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in traversal
                .program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                if let Some(call_site) = find_call_site_in_expression(traversal, field.value) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => None,
    }
}
