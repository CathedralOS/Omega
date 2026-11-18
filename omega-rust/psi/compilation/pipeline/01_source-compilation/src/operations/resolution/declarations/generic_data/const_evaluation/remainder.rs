/*
    Formation checks before closed const arguments or facts lose their source
    operands. A destination's const parameter type cannot type anonymous `%`.
*/

use super::super::{BinaryOperator, SyntaxTrees};

use super::anonymous::anonymous_numeric_expression;
use syntax_trees::expression::TableBinaryExpression;

pub(super) fn validate_anonymous_remainder(
    syntax: &SyntaxTrees,
    binary: &TableBinaryExpression,
) -> Result<(), String> {
    if binary.operator == BinaryOperator::Modulo
        && anonymous_numeric_expression(syntax, binary.left)
        && anonymous_numeric_expression(syntax, binary.right)
    {
        return Err("builtin `%` requires an integer-typed operand; type an operand (for example, `7i32 % 2`), not just the destination".to_owned());
    }
    Ok(())
}
