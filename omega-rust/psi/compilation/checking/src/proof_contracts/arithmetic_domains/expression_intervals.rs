//! Occurrence-bound integer observations shared by checking clients.
//! Each completed walk contributes an interval for its exact expression handle;
//! this module owns the representation and join, while callers retain fact
//! availability and source/storage-version checks.

use typed_trees::expression::ExpressionHandle;

/// The values an integer expression can take, as the arithmetic walk that
/// checked its evaluation found them; `None` is an unbounded end. Every walk of
/// the expression contributes, so the interval holds wherever it is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpressionInterval {
    pub expression: ExpressionHandle,
    pub low: Option<i64>,
    pub high: Option<i64>,
}

/// The retained interval of `expression`, from `intervals` in handle order.
pub fn expression_interval(
    intervals: &[ExpressionInterval],
    expression: ExpressionHandle,
) -> Option<&ExpressionInterval> {
    let position = checked_trees::operators::handle_position(expression);
    intervals
        .binary_search_by_key(&position, |interval| {
            checked_trees::operators::handle_position(interval.expression)
        })
        .ok()
        .map(|found| &intervals[found])
}

/// Join every checked walk for one source occurrence. Each walk may see a
/// different branch environment, so retaining only one walk would overstate
/// what is known at every evaluation of that expression.
pub(crate) fn collect_expression_intervals(
    mut values: Vec<super::expression_analysis::CheckedIntegerValue>,
) -> Vec<ExpressionInterval> {
    values.sort_by_key(|value| checked_trees::operators::handle_position(value.expression));
    let mut intervals: Vec<ExpressionInterval> = Vec::new();
    for value in values {
        match intervals.last_mut() {
            Some(previous) if previous.expression == value.expression => {
                let joined = super::Interval {
                    low: previous.low,
                    high: previous.high,
                }
                .union(value.interval);
                previous.low = joined.low;
                previous.high = joined.high;
            }
            _ => intervals.push(ExpressionInterval {
                expression: value.expression,
                low: value.interval.low,
                high: value.interval.high,
            }),
        }
    }
    intervals
}

/// One interval observation of an exact actual argument. Consumers separately
/// establish whether it is a consequence or a complete public envelope.
/// Descriptor extents describe metadata only, never collection contents.
#[derive(Debug, Clone)]
pub struct PredicateInterval {
    pub argument: ExpressionHandle,
    pub extent: bool,
    pub low: i128,
    pub high: i128,
}
