//! Scalar Boolean structure shared by domain and exit proofs. Leaf custody is
//! supplied by the caller; this module does not replay initializers or choose
//! an arithmetic domain for runtime operations.

use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};

pub(in crate::checks::contracts) use facts::ScalarValue;

/// Closed Boolean facts need no state-entry premise. Resolve no names or
/// runtime leaves, and use only the independently selected builtin meanings.
pub(in crate::checks::contracts) fn closed_boolean_value(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> Option<bool> {
    if !has_builtin_operators(program, operators, expression) {
        return None;
    }
    let ScalarValue::Boolean(value) = evaluate(program, expression, &mut |_| None)? else {
        return None;
    };
    Some(value)
}

pub(in crate::checks::contracts) fn has_builtin_operators(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    expression: ExpressionHandle,
) -> bool {
    if operators.uses.iter().any(|(_, operator_use)| {
        operator_use.expression == expression
            && operator_use.status
                != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            has_builtin_operators(program, operators, binary.left)
                && has_builtin_operators(program, operators, binary.right)
        }
        ExpressionNode::Unary(unary) => has_builtin_operators(program, operators, unary.operand),
        ExpressionNode::Borrow(borrow) => has_builtin_operators(program, operators, borrow.target),
        _ => true,
    }
}

pub(super) fn literal(program: &TypedTrees, expression: ExpressionHandle) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value.value_bignum().map(ScalarValue::Integer),
        ExpressionNode::Boolean(value) => Some(ScalarValue::Boolean(*value)),
        _ => None,
    }
}

pub(in crate::checks::contracts) fn evaluate(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
) -> Option<ScalarValue> {
    evaluate_with_comparisons(program, expression, resolve_leaf, &|_, _| true)
}

/// Comparison meaning is supplied by the consumer independently of leaf
/// values. In particular, the domain prover has no selected-operator evidence
/// for Boolean equality merely because both operands have Boolean values.
pub(super) fn evaluate_with_comparisons(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
) -> Option<ScalarValue> {
    evaluate_with_atoms(
        program,
        expression,
        resolve_leaf,
        comparison_is_admitted,
        &mut |_| None,
    )
}

/// Exact semantic observations can supply Boolean atoms before their retained
/// syntax is decomposed. The observation owns its meaning and subject custody;
/// ordinary scalar comparisons keep their independent admission check.
pub(in crate::checks::contracts) fn evaluate_with_atoms(
    program: &TypedTrees,
    expression: ExpressionHandle,
    resolve_leaf: &mut impl FnMut(ExpressionHandle) -> Option<ScalarValue>,
    comparison_is_admitted: &impl Fn(&ScalarValue, &ScalarValue) -> bool,
    observe_boolean: &mut impl FnMut(ExpressionHandle) -> Option<bool>,
) -> Option<ScalarValue> {
    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    if let Some(value) = literal(program, expression) {
        return Some(value);
    }
    if let Some(value) = observe_boolean(expression) {
        return Some(ScalarValue::Boolean(value));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => evaluate_with_atoms(
            program,
            borrow.target,
            resolve_leaf,
            comparison_is_admitted,
            observe_boolean,
        ),
        ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let ScalarValue::Boolean(value) = evaluate_with_atoms(
                program,
                unary.operand,
                resolve_leaf,
                comparison_is_admitted,
                observe_boolean,
            )?
            else {
                return None;
            };
            Some(ScalarValue::Boolean(!value))
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::And
                    | BinaryOperator::Or
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
            ) =>
        {
            let left = evaluate_with_atoms(
                program,
                binary.left,
                resolve_leaf,
                comparison_is_admitted,
                observe_boolean,
            )?;
            // An unevaluated Boolean operand supplies neither a value premise
            // nor a comparison obligation. Leaf lookup must follow the same
            // selective schedule as the expression it is proving.
            match (&left, binary.operator) {
                (ScalarValue::Boolean(false), BinaryOperator::And) => {
                    return Some(ScalarValue::Boolean(false));
                }
                (ScalarValue::Boolean(true), BinaryOperator::Or) => {
                    return Some(ScalarValue::Boolean(true));
                }
                _ => {}
            }
            let right = evaluate_with_atoms(
                program,
                binary.right,
                resolve_leaf,
                comparison_is_admitted,
                observe_boolean,
            )?;
            if !matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or)
                && !comparison_is_admitted(&left, &right)
            {
                return None;
            }
            let value = match (left, right) {
                (ScalarValue::Boolean(left), ScalarValue::Boolean(right)) => {
                    match binary.operator {
                        BinaryOperator::And => left && right,
                        BinaryOperator::Or => left || right,
                        BinaryOperator::Equal => left == right,
                        BinaryOperator::NotEqual => left != right,
                        _ => return None,
                    }
                }
                (ScalarValue::Integer(left), ScalarValue::Integer(right)) => {
                    match binary.operator {
                        BinaryOperator::Equal => left == right,
                        BinaryOperator::NotEqual => left != right,
                        BinaryOperator::Less => left < right,
                        BinaryOperator::LessOrEqual => left <= right,
                        BinaryOperator::Greater => left > right,
                        BinaryOperator::GreaterOrEqual => left >= right,
                        _ => return None,
                    }
                }
                _ => return None,
            };
            Some(ScalarValue::Boolean(value))
        }
        // Arithmetic, calls, casts, and projections need the caller's exact
        // value evidence. Literal operands do not grant a runtime arithmetic
        // interpretation here (e.g. wrapping u8 addition is not i64 addition).
        _ => resolve_leaf(expression),
    }
}
