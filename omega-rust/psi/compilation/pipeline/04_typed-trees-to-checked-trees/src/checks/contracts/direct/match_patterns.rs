//! Discharge direct comparison obligations from exact dispatch observations.
//! Substitution maps formal leaves to caller operands; display labels never
//! establish equality between different symbols or mutable storage locations.

use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

#[allow(clippy::too_many_arguments)]
pub(super) fn proves(
    program: &TypedTrees,
    subject: ExpressionHandle,
    pattern: ExpressionHandle,
    matched: bool,
    required: ExpressionHandle,
    required_value: bool,
    substitute: &impl Fn(ExpressionHandle) -> ExpressionHandle,
) -> bool {
    proves_scoped(
        program,
        subject,
        pattern,
        matched,
        required,
        required_value,
        substitute,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn proves_scoped(
    program: &TypedTrees,
    subject: ExpressionHandle,
    pattern: ExpressionHandle,
    matched: bool,
    required: ExpressionHandle,
    required_value: bool,
    substitute: &impl Fn(ExpressionHandle) -> ExpressionHandle,
    caller_scope: bool,
) -> bool {
    let replacement = if caller_scope {
        required
    } else {
        substitute(required)
    };
    // A recursive call may use its own formal symbol inside the actual. Once
    // entering that caller expression, never substitute its leaves a second time.
    let caller_scope = caller_scope || replacement != required;
    let required = replacement;
    if matches!(
        program.expression_table.expression(required),
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_)
    ) && !crate::authored_selections::typed_operator_has_no_authored_selection(program, required)
    {
        return false;
    }
    let equal = |left, right| {
        program
            .expression_table
            .expressions_structurally_equal(left, right)
    };
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(pattern)
        && equal(subject, required)
    {
        return (*value == matched) == required_value;
    }
    match program.expression_table.expression(required) {
        ExpressionNode::Boolean(value) => *value == required_value,
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            proves_scoped(
                program,
                subject,
                pattern,
                matched,
                unary.operand,
                !required_value,
                substitute,
                caller_scope,
            )
        }
        ExpressionNode::Binary(binary) => match binary.operator {
            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                let left = if caller_scope {
                    binary.left
                } else {
                    substitute(binary.left)
                };
                let right = if caller_scope {
                    binary.right
                } else {
                    substitute(binary.right)
                };
                ((equal(subject, left) && equal(pattern, right))
                    || (equal(subject, right) && equal(pattern, left)))
                    && (matched == (binary.operator == BinaryOperator::Equal)) == required_value
            }
            BinaryOperator::And | BinaryOperator::Or => {
                let left = || {
                    proves_scoped(
                        program,
                        subject,
                        pattern,
                        matched,
                        binary.left,
                        required_value,
                        substitute,
                        caller_scope,
                    )
                };
                let right = || {
                    proves_scoped(
                        program,
                        subject,
                        pattern,
                        matched,
                        binary.right,
                        required_value,
                        substitute,
                        caller_scope,
                    )
                };
                if (binary.operator == BinaryOperator::And) == required_value {
                    left() && right()
                } else {
                    left() || right()
                }
            }
            _ => false,
        },
        _ => false,
    }
}
