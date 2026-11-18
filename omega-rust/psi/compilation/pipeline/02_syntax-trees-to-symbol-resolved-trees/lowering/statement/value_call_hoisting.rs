/* Capture a shared guard-call subject once without changing terminal invocation position. */

use crate::lowering::expression::expression_table;
use crate::lowering::lowerer::Lowerer;
use crate::lowering::statement::hoist_temps::mint_hoist_temp;
use crate::lowering::statement::indexed_read_hoisting::is_integer_embedding_call;
use crate::lowering::statement::statement_nodes::set_expression;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use symbol_resolved_trees::statement::Statement;
use syntax_trees::{self as syntax, SyntaxTrees};

/*
    In an eligible guard comparison, evaluate a user call once before
    testing its result. For `self.next() == expected`, the other arm
    can reuse the saved result. Typing infers the local from the call.
*/
pub(crate) fn hoist_scalar_value_call_comparison(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    syntax_guard: syntax::statement::TransitionGuardNode,
    guard_expression: ExpressionHandle,
    hoisted: &mut Vec<Statement>,
) {
    let syntax::statement::TransitionGuardNode::When(mut syntax_cmp) = syntax_guard else {
        return;
    };
    let mut resolved_cmp = guard_expression;
    let binary = loop {
        let node = expression_table(lowerer).expression(resolved_cmp).clone();
        let ExpressionNode::Binary(binary) = node else {
            return;
        };
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) && matches!(
            expression_table(lowerer).expression(binary.right),
            ExpressionNode::Boolean(_)
        ) {
            let syntax::expression::ExpressionNode::Binary(syntax_outer) =
                syntax_trees.expressions.expression(syntax_cmp)
            else {
                return;
            };
            resolved_cmp = binary.left;
            syntax_cmp = syntax_outer.left;
            continue;
        }
        break binary;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return;
    }
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let side_is_user_call = |handle: ExpressionHandle| match expressions.expression(handle) {
        ExpressionNode::Call(call) => !is_integer_embedding_call(lowerer, call),
        _ => false,
    };
    let call_is_left = side_is_user_call(binary.left) && !side_is_user_call(binary.right);
    let call_is_right = side_is_user_call(binary.right) && !side_is_user_call(binary.left);
    if !call_is_left && !call_is_right {
        return;
    }
    let other_side = if call_is_left {
        binary.right
    } else {
        binary.left
    };
    if expression_contains(lowerer, other_side, &|node| {
        matches!(node, ExpressionNode::Call(_))
    }) {
        return;
    }
    if call_is_right
        && !matches!(
            expressions.expression(binary.left),
            ExpressionNode::Boolean(_)
                | ExpressionNode::Float(_)
                | ExpressionNode::UndeterminedNaN(_)
                | ExpressionNode::Integer(_)
        )
    {
        return;
    }
    let call_side = if call_is_left {
        binary.left
    } else {
        binary.right
    };

    let syntax_call = match syntax_trees.expressions.expression(syntax_cmp) {
        syntax::expression::ExpressionNode::Binary(syntax_binary) => {
            if call_is_left {
                syntax_binary.left
            } else {
                syntax_binary.right
            }
        }
        _ => syntax_cmp,
    };
    let subject_key = syntax_call;

    let name_reference =
        mint_hoist_temp(lowerer, hoisted, None, Some(subject_key), false, |_, _| {
            call_side
        });
    let rewritten = TableBinaryExpression {
        policy: binary.policy,
        left: if call_is_left {
            name_reference
        } else {
            binary.left
        },
        operator: binary.operator,
        right: if call_is_right {
            name_reference
        } else {
            binary.right
        },
    };
    set_expression(lowerer, resolved_cmp, ExpressionNode::Binary(rewritten));
}

/*
    Whether `expression` evaluates any node `selects`, including nodes inside
    match arms that may run. The walk covers every evaluated child.
*/
pub(super) fn expression_contains(
    lowerer: &Lowerer,
    expression: ExpressionHandle,
    selects: &dyn Fn(&ExpressionNode) -> bool,
) -> bool {
    let expressions = &lowerer.symbol_resolved_trees.tables.bodies.expressions;
    let node = expressions.expression(expression);
    if selects(node) {
        return true;
    }
    let contains = |child| expression_contains(lowerer, child, selects);
    match node {
        ExpressionNode::Match(dispatch) => {
            contains(dispatch.subject)
                || expressions.match_arms(dispatch.arms).iter().any(|arm| {
                    expressions
                        .match_pattern_dependencies(arm.pattern)
                        .any(contains)
                        || contains(arm.value)
                })
        }
        ExpressionNode::SavedSubject(_) => false,
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && contains(call.receiver))
                || expressions
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| contains(*argument))
        }
        ExpressionNode::Atomic(atomic) => contains(atomic.value),
        ExpressionNode::ArrayLiteral(values) => expressions
            .expression_handles(*values)
            .iter()
            .any(|value| contains(*value)),
        ExpressionNode::Binary(binary) => contains(binary.left) || contains(binary.right),
        ExpressionNode::Cast(cast) => contains(cast.value),
        ExpressionNode::Indexed(indexed) => contains(indexed.collection) || contains(indexed.index),
        ExpressionNode::Member(member) => contains(member.receiver),
        ExpressionNode::Membership(membership) => contains(membership.value),
        ExpressionNode::Borrow(borrow) => contains(borrow.target),
        ExpressionNode::Range(range) => contains(range.start) || contains(range.end),
        ExpressionNode::StructLiteral(literal) => expressions
            .struct_fields(literal.fields)
            .iter()
            .any(|field| contains(field.value)),
        ExpressionNode::Unary(unary) => contains(unary.operand),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::TypeExpression(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}
