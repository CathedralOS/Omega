use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::super::super::expressions::expression_integer_value;
use super::super::super::facts::RangeFacts;

/// Mints the ordering fact for a comparison: `strict` records `<` vs `<=`
/// verbatim — a strict ordering lets the bound-chaining consumers accept an
/// upper bound one element higher (`index < j <= len` proves `index < len`).
pub(in crate::checks::ranges::guards) fn seed_at_most_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    lower: ExpressionHandle,
    upper: ExpressionHandle,
    strict: bool,
) {
    let lower_label = program.expression_table.display_name(lower);
    let upper_label = program.expression_table.display_name(upper);
    if strict {
        facts.prove_strictly_less(lower_label.clone(), upper_label);
    } else {
        facts.prove_at_most(lower_label.clone(), upper_label);
    }

    // A bounded subtraction on the upper side relaxes to strictness on the
    // base: `lower < e - k` (any k >= 0) and `lower <= e - k` (k >= 1) both
    // give `lower < e`. (`lower <= e - 0` stays non-strict — it is
    // `lower <= e` verbatim.) `pos == e - 1` reaches the same mint through
    // the equality arm's at-most seed.
    let ExpressionNode::Binary(subtraction) = program.expression_table.expression(upper) else {
        return;
    };
    if subtraction.operator != BinaryOperator::Subtract {
        return;
    }
    let Some(offset) = expression_integer_value(program, facts, subtraction.right) else {
        return;
    };
    if offset < if strict { 0 } else { 1 } {
        return;
    }
    let base = program.expression_table.display_name(subtraction.left);
    facts.prove_strictly_less(lower_label, base);
}

/// Seeds `subject >= 0` from a guard that bounds `subject` below by a constant:
/// `subject >= K` (`inclusive`) or `subject > K` (exclusive, so `subject >= K+1`).
/// When the resulting floor is `>= 0`, `subject` is provably non-negative -- the
/// lower-bound half of a SIGNED index obligation (`0 <= i < len`). The common
/// source is a `self.i >= 0` loop guard.
pub(in crate::checks::ranges::guards) fn seed_non_negative_fact(
    program: &typed_trees::TypedTrees,
    facts: &mut RangeFacts<'_>,
    subject: ExpressionHandle,
    bound: ExpressionHandle,
    inclusive: bool,
) {
    let Some(bound_value) = expression_integer_value(program, facts, bound) else {
        return;
    };
    let floor = if inclusive {
        bound_value
    } else {
        match bound_value.checked_add(1) {
            Some(floor) => floor,
            None => return,
        }
    };
    if floor >= 0 {
        facts.prove_non_negative(program.expression_table.display_name(subject));
    }
}
