//! Capture a shared guard-call subject once without changing terminal invocation position.

use crate::lowering::statement::hoist_temps::mint_hoist_temp;
use crate::lowering::statement::indexed_read_hoisting::is_integer_embedding_call;
use crate::lowering::statement::statement_nodes::set_expression;
use crate::resolution::lowerer::Lowerer;
use symbol_resolved_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression,
};
use symbol_resolved_trees::statement::Statement;
use symbol_resolved_trees::types::TypeReference;
use syntax_trees::{self as syntax, SyntaxTrees};

/// Hoists a USER value-machine call out of a guard comparison:
/// `self.next() == expected` becomes
/// `let __hoist_N = self.next(); __hoist_N == expected`. Fires only when the
/// guard ROOT is a comparison with one user Call side and one non-user-Call
/// side. The
/// temp's type is resolved from the callee's DECLARED return by the
/// symbol-resolved -> typed lowering
/// (`infer_hoist_temp_type`); an inferred-return callee gets a clear
/// annotate-or-bind diagnostic there.
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
    // Peel bool-arm wrappers in LOCKSTEP on both trees: a bool-arm dispatch
    // wraps the subject per arm (`(dbl(5) == 11) == true`), and a match-over-
    // call arm arrives directly as `roll(..) == <arm literal>` with the SAME
    // syntax subject handle shared across arms.
    let mut resolved_cmp = guard_expression;
    let binary = loop {
        let node = lowerer
            .symbol_resolved_trees
            .tables
            .bodies
            .expressions
            .expression(resolved_cmp)
            .clone();
        let ExpressionNode::Binary(binary) = node else {
            return;
        };
        if matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::NotEqual
        ) && matches!(
            lowerer
                .symbol_resolved_trees
                .tables
                .bodies
                .expressions
                .expression(binary.right),
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
    // A cast or nested expression does not hide its call's evaluation point.
    // Keep both operands intact rather than moving one call ahead of the other.
    // Literal match arms still share the existing single subject binding.
    if expression_contains_call(lowerer, other_side) {
        return;
    }
    // A right-side call cannot move ahead of an earlier read or operation:
    // even without another call, that operand can observe storage or trap.
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

    // The memo key is the CALL's SYNTAX handle: a match over a call subject
    // (`transition self.roll(t) { 1 -> .. 2 -> .. }`) lowers one comparison
    // PER ARM over the SAME syntax subject, and every arm must share ONE temp
    // -- per-arm temps re-run the callee once per attempted arm (the
    // effectful-subject single-evaluation tripwire).
    let syntax_call = match syntax_trees.expressions.expression(syntax_cmp) {
        syntax::expression::ExpressionNode::Binary(syntax_binary) => {
            if call_is_left {
                syntax_binary.left
            } else {
                syntax_binary.right
            }
        }
        // A match-over-call arm whose SYNTAX guard is the bare subject (the
        // arm value is synthesized): the subject itself is the call.
        _ => syntax_cmp,
    };
    let subject_key = syntax_call;

    // The inferred temp takes the callee's DECLARED return
    // (`infer_hoist_temp_type`'s Call branch).
    let name_reference = mint_hoist_temp(
        lowerer,
        hoisted,
        TypeReference::Unit,
        Some(subject_key),
        false,
        |_, _| call_side,
    );
    let rewritten = TableBinaryExpression {
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

fn expression_contains_call(lowerer: &Lowerer, expression: ExpressionHandle) -> bool {
    expression_contains(lowerer, expression, &|node| {
        matches!(node, ExpressionNode::Call(_))
    })
}

/// Whether `expression` evaluates any node `selects`, including nodes inside
/// match arms that may run. The walk covers every evaluated child.
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
        | ExpressionNode::ZeroValue(_) => false,
    }
}
