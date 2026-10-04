//! Remainder has no anonymous numeric meaning. Check formation independently
//! of value evaluation, including operands whose anonymous division is fractional.

use diagnostics::Diagnostic;
use language_core::OperatorSpelling;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};

use super::integer_landing::{has_anonymous_operator_meaning, has_builtin_anonymous_operands};

pub(crate) fn validate_anonymous_remainders(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Range bounds and proof facts are not necessarily machine-body expressions.
    // Inspect retained source nodes before any consumer treats a fold as evidence.
    for (expression, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Binary(binary) = node else {
            continue;
        };
        if binary.operator == BinaryOperator::Modulo
            && has_builtin_anonymous_operands(program, expression, OperatorSpelling::Modulo)
            && is_anonymous_numeric_expression(program, binary.left)
            && is_anonymous_numeric_expression(program, binary.right)
        {
            diagnostics.push(
                Diagnostic::error(
                    "builtin `%` requires an integer-typed operand; type an operand (for example, \
                     `7i32 % 2`), not just the destination",
                )
                .with_source_span(program.expression_table.source_span(expression)),
            );
        }
    }
}

pub(super) fn is_anonymous_numeric_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    enum Step {
        Enter(ExpressionHandle),
        Leave,
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    while let Some(step) = pending.pop() {
        let Step::Enter(expression) = step else {
            active.pop();
            continue;
        };
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return false;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
            ExpressionNode::Float(literal) if literal.landing().is_none() => {}
            ExpressionNode::Binary(binary)
                if has_anonymous_operator_meaning(program, expression) =>
            {
                active.push(expression);
                pending.push(Step::Leave);
                pending.push(Step::Enter(binary.right));
                pending.push(Step::Enter(binary.left));
            }
            // Named values, casts, and selected calls establish independent
            // typing boundaries. Their validity belongs to ordinary checking.
            _ => return false,
        }
    }
    true
}
