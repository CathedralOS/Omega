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

fn is_anonymous_numeric_expression(program: &TypedTrees, expression: ExpressionHandle) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::literals::IntegerLiteral;
    use typed_trees::expression::TableBinaryExpression;

    #[test]
    fn anonymous_remainder_formation_is_independent_of_value_and_depth() {
        let mut program = TypedTrees::default();
        let one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let zero = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
        let mut operand = one;
        for _ in 0..600 {
            operand =
                program
                    .expression_table
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: operand,
                        operator: BinaryOperator::Add,
                        right: zero,
                    }));
        }
        let undefined =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left: operand,
                    operator: BinaryOperator::Divide,
                    right: zero,
                }));
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: undefined,
                operator: BinaryOperator::Modulo,
                right: one,
            }));
        let mut diagnostics = Vec::new();
        validate_anonymous_remainders(&program, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("integer-typed operand"));
    }

    #[test]
    fn malformed_and_cyclic_operands_do_not_acquire_anonymous_meaning() {
        let mut program = TypedTrees::default();
        let one = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        assert!(!is_anonymous_numeric_expression(
            &program,
            ExpressionHandle::invalid()
        ));
        assert!(!is_anonymous_numeric_expression(
            &program,
            ExpressionHandle::from_parts(one.arena_index(), one.generation() + 1)
        ));
        *program.expression_table.expression_mut(one) =
            ExpressionNode::Binary(TableBinaryExpression {
                left: one,
                operator: BinaryOperator::Add,
                right: one,
            });
        assert!(!is_anonymous_numeric_expression(&program, one));
    }
}
