//! Atomic binding expansion after the ordinary `let` grammar has parsed once.

use crate::bodies::statements::statement_tables::copy_expression_as_place;
use crate::expressions::parse_postfix::memory_ordering_from_expression;
use arena::HandleSpan;
use language_core::atomic::AtomicOrderingPlan;
use numerics::literals::IntegerLiteral;
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableAtomicExpression, TableBinaryExpression,
};
use syntax_trees::identifier::Identifier;
use syntax_trees::statement::{StatementHandle, StatementNode, TableAssignment, TableLocalData};

enum AtomicUpdate {
    Fetch {
        operand: ExpressionHandle,
        operator: BinaryOperator,
    },
    Swap {
        replacement: ExpressionHandle,
    },
    CompareExchange {
        expected: ExpressionHandle,
        replacement: ExpressionHandle,
    },
}

/// Classify an already parsed initializer, then publish the result binding and
/// atomic write together. Unrecognized calls leave the ordinary binding intact.
/// No statements are published until the complete operation and place match.
pub(crate) fn try_desugar_atomic_let(
    syntax_trees: &mut SyntaxTrees,
    binding: &TableLocalData,
) -> Option<HandleSpan<StatementHandle>> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(binding.initial_value)
    else {
        return None;
    };
    let arguments = syntax_trees.expressions.expression_handles(call.arguments);
    let (update, ordering) = match (call.target.as_str(), arguments) {
        (
            "fetch_add" | "fetch_sub" | "fetch_xor" | "fetch_or" | "fetch_and",
            [operand, ordering],
        ) => {
            let operator = match call.target.as_str() {
                "fetch_add" => BinaryOperator::Add,
                "fetch_sub" => BinaryOperator::Subtract,
                "fetch_xor" => BinaryOperator::BitwiseXor,
                "fetch_or" => BinaryOperator::BitwiseOr,
                "fetch_and" => BinaryOperator::BitwiseAnd,
                _ => return None,
            };
            (
                AtomicUpdate::Fetch {
                    operand: *operand,
                    operator,
                },
                AtomicOrderingPlan::ReadModifyWrite(
                    memory_ordering_from_expression(syntax_trees, *ordering).ok()?,
                ),
            )
        }
        ("swap", [replacement, ordering]) => (
            AtomicUpdate::Swap {
                replacement: *replacement,
            },
            AtomicOrderingPlan::Swap(
                memory_ordering_from_expression(syntax_trees, *ordering).ok()?,
            ),
        ),
        ("compare_exchange", [expected, replacement, success, failure]) => (
            AtomicUpdate::CompareExchange {
                expected: *expected,
                replacement: *replacement,
            },
            AtomicOrderingPlan::CompareExchange {
                success: memory_ordering_from_expression(syntax_trees, *success).ok()?,
                failure: memory_ordering_from_expression(syntax_trees, *failure).ok()?,
            },
        ),
        _ => return None,
    };
    let target = copy_expression_as_place(syntax_trees, call.receiver)?;

    // Reserve storage without reading the atomic place. The selected instruction
    // supplies the observed prior value; a separate load would race with it.
    let mut result_binding = binding.clone();
    result_binding.initial_value = syntax_trees
        .expressions
        .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
    let result = result_reference(syntax_trees, &binding.name);
    let value =
        match update {
            AtomicUpdate::Fetch { operand, operator } => {
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: result,
                        operator,
                        right: operand,
                    }))
            }
            AtomicUpdate::Swap { replacement } => replacement,
            AtomicUpdate::CompareExchange {
                expected,
                replacement,
            } => {
                // Preserve the existing interpreter model:
                // prior + (prior == expected) * (replacement - prior).
                // Native selection emits one CAS and stores its observed prior.
                let prior_for_subtract = result_reference(syntax_trees, &binding.name);
                let difference = syntax_trees.expressions.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        left: replacement,
                        operator: BinaryOperator::Subtract,
                        right: prior_for_subtract,
                    },
                ));
                let prior_for_compare = result_reference(syntax_trees, &binding.name);
                let comparison = syntax_trees.expressions.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        left: prior_for_compare,
                        operator: BinaryOperator::Equal,
                        right: expected,
                    },
                ));
                let replacement_delta = syntax_trees.expressions.insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        left: comparison,
                        operator: BinaryOperator::Multiply,
                        right: difference,
                    },
                ));
                syntax_trees
                    .expressions
                    .insert(ExpressionNode::Binary(TableBinaryExpression {
                        left: result,
                        operator: BinaryOperator::Add,
                        right: replacement_delta,
                    }))
            }
        };
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value,
            result,
            ordering,
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        }));
    let local = syntax_trees
        .statements
        .insert(StatementNode::LocalData(result_binding));
    let first = syntax_trees.items.append_statement_handle(local);
    let assignment = syntax_trees
        .statements
        .insert(StatementNode::Assignment(TableAssignment { target, value }));
    syntax_trees.items.append_statement_handle(assignment);
    Some(HandleSpan::from_parts(first, 2))
}

fn result_reference(syntax_trees: &mut SyntaxTrees, name: &Identifier) -> ExpressionHandle {
    let member = syntax_trees
        .expressions
        .append_identifier_path_member(Identifier::generated(name.as_str()));
    let path = HandleSpan::from_parts(member, 1);
    syntax_trees.expressions.insert(ExpressionNode::Name(path))
}

/// Carry an atomic store as an assignment without changing its operand evaluation.
pub(crate) fn try_desugar_atomic_store(
    syntax_trees: &mut SyntaxTrees,
    expression: ExpressionHandle,
) -> Option<TableAssignment> {
    let ExpressionNode::Call(call) = syntax_trees.expressions.expression(expression).clone() else {
        return None;
    };
    if call.target.as_str() != "store" || !call.receiver.is_valid() {
        return None;
    }
    let argument_count = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments)
        .len();
    if argument_count != 2 {
        // Not the atomic store shape (wrong arity); fall through to normal
        // call-statement or error path.
        return None;
    }
    let arguments = syntax_trees
        .tables
        .expressions
        .expression_handles(call.arguments);
    let value = arguments[0];
    let ordering = memory_ordering_from_expression(syntax_trees, arguments[1]).ok()?;
    let value = syntax_trees
        .expressions
        .insert(ExpressionNode::Atomic(TableAtomicExpression {
            value,
            result: ExpressionHandle::invalid(),
            ordering: language_core::atomic::AtomicOrderingPlan::Store(ordering),
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        }));
    let receiver = call.receiver;
    Some(TableAssignment {
        target: receiver,
        value,
    })
}
