use typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::calls::collect_state_argument_facts_for_call;
use super::{StateArgumentContext, StateArgumentFacts};
use crate::checks::ranges::facts::RangeFacts;

pub(super) fn collect_state_argument_facts_from_expression(
    context: &StateArgumentContext<'_, '_>,
    facts: &mut RangeFacts<'_>,
    expression: ExpressionHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    if !expression.is_valid() {
        return;
    }
    let program = context.program;
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
        collect_state_argument_facts_from_expression(context, facts, call.receiver, collected);
        let arguments = program.expression_table.expression_handles(call.arguments);
        for argument in arguments {
            collect_state_argument_facts_from_expression(context, facts, *argument, collected);
        }
        collect_state_argument_facts_for_call(
            program,
            context.machine,
            context.state,
            facts,
            call.target_symbol,
            arguments,
            collected,
        );
        let paths = context.call_frames.and_then(|frames| {
            frames
                .expression_write_frame(context.machine, expression)
                .into_complete_paths()
        });
        facts.invalidate_call_writes(
            program,
            context.machine,
            context.state,
            paths.as_deref(),
            Some(&crate::CallSite::Expression { expression, call }),
        );
        return;
    }
    let mut visit = |child| {
        collect_state_argument_facts_from_expression(context, facts, child, collected);
    };
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => visit(atomic.value),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                visit(*value);
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left);
            visit(binary.right);
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => {
            visit(indexed.collection);
            visit(indexed.index);
        }
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::Borrow(borrow) => visit(borrow.target),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Range(range) => {
            visit(range.start);
            visit(range.end);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                visit(field.value);
            }
        }
        ExpressionNode::Call(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}
