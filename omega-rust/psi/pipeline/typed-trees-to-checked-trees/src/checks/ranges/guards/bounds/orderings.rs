use typed_trees::expression::ExpressionHandle;

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
    let lower = program.expression_table.display_name(lower);
    let upper = program.expression_table.display_name(upper);
    if strict {
        facts.prove_strictly_less(lower, upper);
    } else {
        facts.prove_at_most(lower, upper);
    }
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
