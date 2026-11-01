//! Traversal arms identical across the borrow walks. A child-bearing node
//! recurses through `visit`; any other node returns `false` so the caller
//! classifies it under its own rules.

use crate::flow::expressions;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};

/// Recurse `visit` over the child expressions the borrow walks share:
/// match subjects/patterns/values, atomics, array/range/unary operands,
/// binary operands, cast values, and struct-literal fields. Returns
/// `false` for nodes the walks classify differently.
pub(super) fn recurse_shared_children(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    visit: &mut dyn FnMut(ExpressionHandle),
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Match(dispatch) => {
            visit(dispatch.subject);
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for pattern in program.expression_table.match_pattern_operands(arm.pattern) {
                    visit(pattern);
                }
                visit(arm.value);
            }
        }
        ExpressionNode::Atomic(atomic) => {
            for child in validation::atomic_execution_children(program, atomic) {
                visit(child);
            }
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                visit(*value);
            }
        }
        ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                visit(range.start);
            }
            if range.end.is_valid() {
                visit(range.end);
            }
        }
        ExpressionNode::Binary(binary) => {
            visit(binary.left);
            // Loan occurrences follow the same short-circuit prefix as flow.
            // A proved false tag must not allocate a call ordinal for a guard
            // that cannot execute; reachable guards retain their ordinary loans.
            let skipped = matches!(
                (binary.operator, program.static_boolean_value(binary.left)),
                (typed_trees::expression::BinaryOperator::And, Some(false))
                    | (typed_trees::expression::BinaryOperator::Or, Some(true))
            );
            if !skipped {
                visit(binary.right);
            }
        }
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions(program).struct_fields(struct_literal.fields) {
                visit(field.value);
            }
        }
        _ => return false,
    }
    true
}
