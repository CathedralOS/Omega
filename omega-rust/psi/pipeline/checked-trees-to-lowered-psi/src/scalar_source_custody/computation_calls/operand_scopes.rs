//! Source operand coordinates, independent of arithmetic template meaning.

use super::*;
use checked_trees::expression::BinaryOperator;

pub(super) fn application(
    checked: &CheckedTrees,
    mut source: ExpressionHandle,
    arity: usize,
) -> Result<Vec<ExpressionHandle>, LoweringError> {
    let mut visited = Vec::new();
    loop {
        if !checked.expression_table.expression_is_valid(source) || visited.contains(&source) {
            return unsupported("computed application has a stale or cyclic source");
        }
        visited.push(source);
        match checked.expression_table.expression(source) {
            ExpressionNode::Binary(binary)
                if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) =>
            {
                // Logical selection is a Select, never an Apply. When its
                // condition was folded, this application belongs to the RHS.
                // This locates operands; it does not establish a guard proof.
                source = binary.right;
            }
            ExpressionNode::Binary(binary) if arity == 2 => {
                // Comparison normalization reverses values, not evaluation.
                return Ok(vec![binary.left, binary.right]);
            }
            ExpressionNode::Unary(unary) if arity == 1 => return Ok(vec![unary.operand]),
            ExpressionNode::Cast(cast) if arity == 1 => return Ok(vec![cast.value]),
            // A same-carrier cast changes policy without adding an operation.
            ExpressionNode::Cast(cast) => source = cast.value,
            _ => return unsupported("computed application lost its authored operand positions"),
        }
    }
}

pub(super) fn selection(
    checked: &CheckedTrees,
    source: ExpressionHandle,
) -> Result<(ExpressionHandle, ExpressionHandle, bool), LoweringError> {
    if let ExpressionNode::Binary(binary) = checked.expression_table.expression(source) {
        match binary.operator {
            BinaryOperator::And => return Ok((binary.left, binary.right, true)),
            BinaryOperator::Or => return Ok((binary.left, binary.right, false)),
            _ => {}
        }
    }
    unsupported("computed selection lost its authored conditional operands")
}
