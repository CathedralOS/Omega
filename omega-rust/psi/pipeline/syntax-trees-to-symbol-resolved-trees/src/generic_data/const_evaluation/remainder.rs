//! Formation checks before closed const arguments or facts lose their source
//! operands. A destination's const parameter type cannot type anonymous `%`.

use super::*;
use syntax_trees::expression::TableBinaryExpression;
use syntax_trees::operator_spelling::OperatorSpelling;

pub(super) fn validate_anonymous_remainder(
    syntax: &SyntaxTrees,
    binary: &TableBinaryExpression,
) -> Result<(), String> {
    if binary.operator == BinaryOperator::Modulo
        && has_no_authored_spelling(syntax, OperatorSpelling::Modulo)
        && anonymous_numeric_expression(syntax, binary.left)
        && anonymous_numeric_expression(syntax, binary.right)
    {
        return Err("builtin `%` requires an integer-typed operand; type an operand (for example, `7i32 % 2`), not just the destination".to_owned());
    }
    Ok(())
}

fn has_no_authored_spelling(syntax: &SyntaxTrees, spelling: OperatorSpelling) -> bool {
    // This pre-resolution evaluator has no selected-operator authority. Any
    // potentially relevant authored spelling makes this narrow builtin check
    // decline; it does not authorize that declaration or certify the existing
    // const evaluator's handling of authored operator meanings.
    !syntax.root_items().any(|item| match item {
        Item::Operator(operator) => operator.spelling == Some(spelling),
        Item::Domain(domain) => syntax
            .items
            .operators(domain.operators)
            .iter()
            .any(|operator| operator.spelling == Some(spelling)),
        _ => false,
    })
}

fn anonymous_numeric_expression(syntax: &SyntaxTrees, expression: ExpressionHandle) -> bool {
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
        if !expression.is_valid() || active.contains(&expression) {
            return false;
        }
        match syntax.expressions.expression(expression) {
            ExpressionNode::Integer(literal) if literal.landing().is_none() => {}
            ExpressionNode::Binary(binary) => {
                let spelling = match binary.operator {
                    BinaryOperator::Add => OperatorSpelling::Add,
                    BinaryOperator::Subtract => OperatorSpelling::Subtract,
                    BinaryOperator::Multiply => OperatorSpelling::Multiply,
                    BinaryOperator::Divide => OperatorSpelling::Divide,
                    _ => return false,
                };
                if !has_no_authored_spelling(syntax, spelling) {
                    return false;
                }
                active.push(expression);
                pending.push(Step::Leave);
                pending.push(Step::Enter(binary.right));
                pending.push(Step::Enter(binary.left));
            }
            _ => return false,
        }
    }
    true
}
