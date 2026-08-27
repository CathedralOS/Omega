use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;

use super::StateArgumentFacts;
use super::calls::collect_state_argument_facts_for_call;
use crate::checks::ranges::facts::RangeFacts;

pub(super) fn collect_state_argument_facts_from_expression(
    program: &psi_typed_trees::TypedTrees,
    machine: &Machine,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
    collected: &mut Vec<StateArgumentFacts>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => collect_state_argument_facts_from_expression(
            program,
            machine,
            facts,
            atomic.value,
            collected,
        ),
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_state_argument_facts_from_expression(
                    program, machine, facts, *value, collected,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                binary.left,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                binary.right,
                collected,
            );
        }
        ExpressionNode::Call(call) => {
            collect_state_argument_facts_for_call(
                program,
                machine,
                facts,
                call.target_symbol,
                Some(&call.target),
                program.expression_table.expression_handles(call.arguments),
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                call.receiver,
                collected,
            );
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_state_argument_facts_from_expression(
                    program, machine, facts, *argument, collected,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_state_argument_facts_from_expression(
                program, machine, facts, cast.value, collected,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                indexed.collection,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                indexed.index,
                collected,
            );
        }
        ExpressionNode::Member(member) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                member.receiver,
                collected,
            );
        }
        ExpressionNode::Borrow(inner) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                inner.target,
                collected,
            );
        }
        ExpressionNode::Unary(unary) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                unary.operand,
                collected,
            );
        }
        ExpressionNode::Range(range) => {
            collect_state_argument_facts_from_expression(
                program,
                machine,
                facts,
                range.start,
                collected,
            );
            collect_state_argument_facts_from_expression(
                program, machine, facts, range.end, collected,
            );
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_state_argument_facts_from_expression(
                    program,
                    machine,
                    facts,
                    field.value,
                    collected,
                );
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
