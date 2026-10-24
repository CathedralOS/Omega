//! The closed interval a guard states for one spelled place.
//!
//! Label routes discharge a requirement from a transition guard by reading the
//! guard's comparisons as an interval, so `index < 16` establishes
//! `requires self <= 15` without matching text. The interval is validation's
//! `BigIntInterval`: an absent end is unbounded, and `&&` meets two readings
//! on that lattice. A conjunct that does not name this spelling contributes
//! no reading. A subject/literal pair that is not a closed comparison
//! contributes the unbounded interval, so `&&` keeps the other conjunct.
//! Either way the result is only ever weaker than the guard.
//! An argument's own interval is not computed here; the arithmetic walk bounds
//! every evaluated argument, and the call checks read that
//! (`validation::expression_interval`).
//!
//! Predicate-only domain requirements still publish an unstated end as
//! `i64::MIN` or `i64::MAX` (`predicate_only_domain_interval`). Containment
//! against that publication treats an absent guard end as the same extreme, so
//! a single comparison discharges the same requirements it did when this
//! module filled those extremes itself. That adapter is not a second lattice:
//! the meet and the ends live on `BigIntInterval`.

use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use validation::BigIntInterval;

/// The closed interval a guard's conjuncts establish for the place spelled
/// `subject_label`.
pub(super) fn guard_interval_for_label(
    program: &dyn ::validation::CheckingInput,
    guard: ExpressionHandle,
    subject_label: &str,
) -> Option<BigIntInterval> {
    let Some(binary) = program.expression_table.binary_node(guard) else {
        return None;
    };
    match binary.operator {
        BinaryOperator::And => {
            let left = guard_interval_for_label(program, binary.left, subject_label);
            let right = guard_interval_for_label(program, binary.right, subject_label);
            match (left, right) {
                (Some(left), Some(right)) => Some(left.intersect(&right)),
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

/// Whether `guard` is contained in the closed requirement whose unstated ends
/// are still the signed extremes. An absent guard end meets a required end
/// only at or beyond that extreme, matching `i64::MIN >= required_low` and
/// `i64::MAX <= required_high`.
pub(super) fn establishes_closed_requirement(
    guard: &BigIntInterval,
    required_low: &BigInt,
    required_high: &BigInt,
) -> bool {
    let low_holds = match &guard.low {
        Some(low) => low >= required_low,
        None => required_low <= &BigInt::from_i64(i64::MIN),
    };
    let high_holds = match &guard.high {
        Some(high) => high <= required_high,
        None => required_high >= &BigInt::from_i64(i64::MAX),
    };
    low_holds && high_holds
}

/// Narrow an open `i64` lattice by a guard interval. An end that does not fit
/// strictly inside `i64::MIN..i64::MAX` stays open: the lattice's own extremes
/// add no stricter finite bound, and a magnitude outside `i64` cannot land in it.
pub(super) fn narrow_i64_bounds(
    mut low: Option<i64>,
    mut high: Option<i64>,
    guard: &BigIntInterval,
) -> (Option<i64>, Option<i64>) {
    if let Some(bound) = guard
        .low
        .as_ref()
        .and_then(BigInt::to_i64)
        .filter(|bound| *bound > i64::MIN)
    {
        low = Some(low.map_or(bound, |current| current.max(bound)));
    }
    if let Some(bound) = guard
        .high
        .as_ref()
        .and_then(BigInt::to_i64)
        .filter(|bound| *bound < i64::MAX)
    {
        high = Some(high.map_or(bound, |current| current.min(bound)));
    }
    (low, high)
}

/// The interval one comparison of `subject_label` against a closed integer
/// literal states. A subject/literal pair that is not a closed comparison
/// states the unbounded interval: `&&` then keeps the other conjunct, and a
/// lone non-comparison does not invent a finite end.
fn comparison_bound(
    program: &dyn ::validation::CheckingInput,
    binary: &typed_trees::expression::TableBinaryExpression,
    subject_label: &str,
) -> Option<BigIntInterval> {
    let left_is_subject = program.expression_table.display_name(binary.left) == subject_label;
    let right_is_subject = program.expression_table.display_name(binary.right) == subject_label;
    let (literal, subject_on_left) = match (left_is_subject, right_is_subject) {
        (true, false) => (binary.right, true),
        (false, true) => (binary.left, false),
        _ => return None,
    };
    let value = validation::closed_integer_range_bound(program, literal)?;
    let (low, high) =
        validation::closed_interval_from_comparison(binary.operator, subject_on_left, value)
            .unwrap_or((None, None));
    Some(BigIntInterval { low, high })
}
