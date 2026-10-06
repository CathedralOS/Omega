//! The closed interval a guard states for one spelled place.
//!
//! The label routes that discharge a requirement from a transition guard read
//! the guard's comparisons as an interval, so `index < 16` establishes
//! `requires self <= 15` without matching text. An argument's own interval is
//! not computed here: the arithmetic walk bounds every evaluated argument, and
//! the call checks read that (`validation::expression_interval`).

use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

/// The closed interval a guard's conjuncts establish for the place spelled
/// `subject_label`. `&&` intersects; a conjunct that is not a closed
/// comparison over that exact spelling contributes nothing, so the result is
/// only ever weaker than the guard.
pub(super) fn guard_interval_for_label(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
    subject_label: &str,
) -> Option<(BigInt, BigInt)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            let left = guard_interval_for_label(program, binary.left, subject_label);
            let right = guard_interval_for_label(program, binary.right, subject_label);
            match (left, right) {
                (Some((left_low, left_high)), Some((right_low, right_high))) => {
                    Some((left_low.max(right_low), left_high.min(right_high)))
                }
                (bound, None) | (None, bound) => bound,
            }
        }
        // A dispatch arm reaches here as `<predicate> == true`.
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            guard_interval_for_label(program, binary.left, subject_label)
        }
        _ => comparison_bound(program, binary, subject_label),
    }
}

/// The comparison `!` distributes over, under builtin operator semantics: the
/// complement that the negated (continuation) arm's `!guard` states.
/// `!(a && b)` refutes an unspecified operand, so it carries no complement.
pub(super) fn complement_comparison(operator: BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        BinaryOperator::Equal => Some(BinaryOperator::NotEqual),
        BinaryOperator::NotEqual => Some(BinaryOperator::Equal),
        BinaryOperator::Less => Some(BinaryOperator::GreaterOrEqual),
        BinaryOperator::LessOrEqual => Some(BinaryOperator::Greater),
        BinaryOperator::Greater => Some(BinaryOperator::LessOrEqual),
        BinaryOperator::GreaterOrEqual => Some(BinaryOperator::Less),
        _ => None,
    }
}

/// The closed interval `!guard` establishes for the place spelled
/// `subject_label` -- the negated-arm read of `guard_interval_for_label`.
/// `!(a || b)` keeps both refuted operands and intersects exactly as `&&`
/// does positively; `!(a && b)` cannot isolate an operand, so it contributes
/// nothing.
pub(super) fn negated_guard_interval_for_label(
    program: &typed_trees::TypedTrees,
    guard: ExpressionHandle,
    subject_label: &str,
) -> Option<(BigInt, BigInt)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::Or => {
            let left = negated_guard_interval_for_label(program, binary.left, subject_label);
            let right = negated_guard_interval_for_label(program, binary.right, subject_label);
            match (left, right) {
                (Some((left_low, left_high)), Some((right_low, right_high))) => {
                    Some((left_low.max(right_low), left_high.min(right_high)))
                }
                (bound, None) | (None, bound) => bound,
            }
        }
        BinaryOperator::Equal
            if matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            negated_guard_interval_for_label(program, binary.left, subject_label)
        }
        _ => {
            let operator = complement_comparison(binary.operator)?;
            // `!(a != b)` spells `a == b` -- a point bound. `!(a == b)`
            // spells `a != b`, which bounds nothing closed.
            (operator != BinaryOperator::NotEqual)
                .then(|| comparison_bound_with_operator(program, binary, operator, subject_label))?
        }
    }
}

/// One comparison's contribution, when exactly one side is the subject and the
/// other is a closed integer literal.
fn comparison_bound(
    program: &typed_trees::TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
    subject_label: &str,
) -> Option<(BigInt, BigInt)> {
    comparison_bound_with_operator(program, binary, binary.operator, subject_label)
}

/// `comparison_bound` under a caller-selected operator, so the negated arm
/// can read the complement comparison the same way.
fn comparison_bound_with_operator(
    program: &typed_trees::TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
    operator: BinaryOperator,
    subject_label: &str,
) -> Option<(BigInt, BigInt)> {
    let left_is_subject = program.expression_table.display_name(binary.left) == subject_label;
    let right_is_subject = program.expression_table.display_name(binary.right) == subject_label;
    let (literal, subject_on_left) = match (left_is_subject, right_is_subject) {
        (true, false) => (binary.right, true),
        (false, true) => (binary.left, false),
        _ => return None,
    };
    let value = validation::closed_integer_range_bound(program, literal)?;
    Some(endpoints(operator, subject_on_left, value))
}

/// The interval one comparison states, with the unconstrained side left at the
/// carrier-independent extreme.
fn endpoints(operator: BinaryOperator, subject_on_left: bool, value: BigInt) -> (BigInt, BigInt) {
    let one = BigInt::from_i64(1);
    let (low, high) = match (operator, subject_on_left) {
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
            (None, Some(value))
        }
        (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
            (None, Some(value.sub(&one)))
        }
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
            (Some(value), None)
        }
        (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
            (Some(value.add(&one)), None)
        }
        (BinaryOperator::Equal, _) => (Some(value.clone()), Some(value)),
        _ => (None, None),
    };
    (
        low.unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
        high.unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
    )
}
