//! Abstract callable contracts have a parameter telescope, not an executable
//! machine. Rejoin each operator with that declaration's selected meaning;
//! this query grants neither a predicate nor argument snapshot evidence.

use language_core::OperatorSpelling;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;

pub fn has_builtin_parameter_bound_expression_meaning(
    program: &TypedTrees,
    owner_symbol: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    let mut pending = vec![(expression, false)];
    let mut visited = Vec::new();
    let mut active = Vec::new();
    while let Some((expression, completed)) = pending.pop() {
        if completed {
            active.pop();
            visited.push(expression);
            continue;
        }
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if visited.contains(&expression) {
            continue;
        }
        if active.contains(&expression) {
            return false;
        }
        active.push(expression);
        pending.push((expression, true));
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                if binary.operator == BinaryOperator::CaseMembership {
                    return false;
                }
                let spelling = spelled_binary_operator(binary.operator);
                if let Some(spelling) = spelling {
                    let operands = [binary.left, binary.right].map(|operand| {
                        crate::parameter_expression_result_type_reference(
                            program,
                            owner_symbol,
                            parameters,
                            operand,
                        )
                    });
                    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        owner_symbol,
                        expression,
                        spelling,
                        &operands,
                    ) {
                        return false;
                    }
                }
                pending.extend([(binary.left, false), (binary.right, false)]);
            }
            ExpressionNode::Unary(unary) => pending.push((unary.operand, false)),
            ExpressionNode::Borrow(borrow) => pending.push((borrow.target, false)),
            ExpressionNode::Cast(cast) => pending.push((cast.value, false)),
            ExpressionNode::Atomic(atomic) => pending.push((atomic.value, false)),
            // Calls and places remain symbolic leaves. Consumers separately
            // check their carrier, invocation and captured value identity.
            _ => {}
        }
    }
    true
}

/// The spelling an authored binary operator supplies to overloadable
/// meaning; membership and the non-overloadable operators have none.
pub(crate) fn spelled_binary_operator(
    operator: BinaryOperator,
) -> Option<OperatorSpelling> {
    match operator {
        BinaryOperator::Equal => Some(OperatorSpelling::Equal),
        BinaryOperator::NotEqual => Some(OperatorSpelling::NotEqual),
        BinaryOperator::Less => Some(OperatorSpelling::Less),
        BinaryOperator::LessOrEqual => Some(OperatorSpelling::LessEqual),
        BinaryOperator::Greater => Some(OperatorSpelling::Greater),
        BinaryOperator::GreaterOrEqual => Some(OperatorSpelling::GreaterEqual),
        BinaryOperator::Add => Some(OperatorSpelling::Add),
        BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
        BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
        BinaryOperator::Divide => Some(OperatorSpelling::Divide),
        BinaryOperator::Modulo => Some(OperatorSpelling::Modulo),
        BinaryOperator::CaseMembership
        | BinaryOperator::And
        | BinaryOperator::Or
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}
