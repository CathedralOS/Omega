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
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
    subject_label: &str,
) -> Option<(BigInt, BigInt)> {
    let Some(binary) = program.expression_table.binary_node(guard) else {
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
        _ => comparison_bound(program, binary, binary.operator, subject_label),
    }
}

/// `comparison_bound` under a caller-selected operator, so the negated arm
/// can read the complement comparison the same way.
fn comparison_bound(
    program: &dyn ::validation::CheckingInput,
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
    let (low, high) =
        ::validation::closed_interval_from_comparison(operator, subject_on_left, value)
            .unwrap_or((None, None));
    (
        low.unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
        high.unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
    )
}
