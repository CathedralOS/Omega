//! The closed integer interval a value can be proven to occupy.
//!
//! One reader for every site that decides DOMAIN MEMBERSHIP, so a bound stated
//! as a domain is discharged the same way wherever it is met: a call argument,
//! a write, a local initializer. The guards a site can see differ -- a
//! transition arm has its own, a statement has the incoming ones -- so they
//! arrive as a slice and intersect.

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

/// One comparison's contribution, when exactly one side is the subject and the
/// other is a closed integer literal.
fn comparison_bound(
    program: &typed_trees::TypedTrees,
    binary: &typed_trees::expression::TableBinaryExpression,
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
    Some(endpoints(binary.operator, subject_on_left, value))
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

/// The interval `expression` can be proven to occupy, or `None` when it cannot
/// be read as a closed one.
///
/// A name contributes its DECLARED interval -- a bracketed range or, since
/// domains carry their bound, a declared domain -- intersected with whatever
/// the supplied guards say about that same spelling. A literal is exact, and
/// `+`/`-` against a literal shifts the interval. Anything else declines, so
/// the result is only ever narrower than the truth.
pub(super) fn expression_interval(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    guards: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<(BigInt, BigInt)> {
    if let Some(value) = validation::closed_integer_range_bound(program, expression) {
        return Some((value.clone(), value));
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
            let label = program.expression_table.display_name(expression);
            let declared = crate::checks::ranges::types::expression_enforced_declared_range(
                program, machine, state, expression,
            )
            .map(|(low, high)| (BigInt::from_i64(low), BigInt::from_i64(high)));
            let guarded = guards
                .iter()
                .filter_map(|guard| guard_interval_for_label(program, *guard, &label))
                .reduce(|(low, high), (other_low, other_high)| {
                    (low.max(other_low), high.min(other_high))
                });
            match (declared, guarded) {
                (Some((declared_low, declared_high)), Some((guarded_low, guarded_high))) => Some((
                    declared_low.max(guarded_low),
                    declared_high.min(guarded_high),
                )),
                (Some(interval), None) | (None, Some(interval)) => Some(interval),
                (None, None) => None,
            }
        }
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract
            ) =>
        {
            let (low, high) = expression_interval(program, machine, state, guards, binary.left)?;
            let shift = validation::closed_integer_range_bound(program, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some((low.add(&shift), high.add(&shift))),
                _ => Some((low.sub(&shift), high.sub(&shift))),
            }
        }
        _ => None,
    }
}
