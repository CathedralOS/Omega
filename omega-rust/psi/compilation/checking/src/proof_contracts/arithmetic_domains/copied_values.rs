//! Integer facts carried by a copied value, independent of its old storage.
//!
//! Scalar leaves consume the intervals already established by expression
//! checking. Record and array constructors compose those leaves, and ordinary
//! aggregate copies translate their retained descendants. Capture precedes
//! destination invalidation, so overlapping stores cannot erase the new value.
//! The supplying environment has already crossed value-evaluation effects;
//! this owner never replays an initializer or independently summarizes calls.

use super::{Interval, ValueEnvironment, place_path};
use std::collections::BTreeMap;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

#[derive(Default)]
pub(crate) struct CopiedIntegerValue {
    intervals: Vec<(String, Interval)>,
    unsigned_values: Vec<(String, u64)>,
}

impl CopiedIntegerValue {
    pub(crate) fn capture(
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        environment: &ValueEnvironment,
        checked_intervals: &[(ExpressionHandle, Interval)],
    ) -> Self {
        let mut value = Self::default();
        value.capture_at(program, expression, "", environment, checked_intervals);
        value
    }

    fn capture_at(
        &mut self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
        suffix: &str,
        environment: &ValueEnvironment,
        checked_intervals: &[(ExpressionHandle, Interval)],
    ) {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    self.capture_at(
                        program,
                        field.value,
                        &format!("{suffix}.{}", field.name),
                        environment,
                        checked_intervals,
                    );
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for (index, element) in program
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .enumerate()
                {
                    self.capture_at(
                        program,
                        *element,
                        &format!("{suffix}[{index}]"),
                        environment,
                        checked_intervals,
                    );
                }
            }
            _ => {
                if let Some(source) = place_path(program, expression)
                    .or_else(|| super::element_place_path(program, expression))
                {
                    copy_retained(&environment.intervals, &source, suffix, &mut self.intervals);
                    copy_retained(
                        &environment.known_u64_values,
                        &source,
                        suffix,
                        &mut self.unsigned_values,
                    );
                }
                if !suffix.is_empty()
                    && let Some(interval) = checked_intervals
                        .iter()
                        .filter_map(|(checked, interval)| {
                            (*checked == expression).then_some(*interval)
                        })
                        .reduce(Interval::union)
                {
                    self.intervals.push((suffix.to_owned(), interval));
                }
                if let Some(value) = super::integer_ranges::literal_u64(program, expression) {
                    self.unsigned_values.push((suffix.to_owned(), value));
                }
            }
        }
    }

    /// The destination's old facts must already have been retired by the
    /// ordinary store owner. A copied root interval is recorded there with its
    /// enforced declared range; this owner supplies the composite descendants.
    pub(crate) fn record(&self, environment: &mut ValueEnvironment, path: Option<String>) {
        let Some(path) = path else { return };
        for (suffix, interval) in &self.intervals {
            if !suffix.is_empty() {
                environment.set(format!("{path}{suffix}"), *interval);
            }
        }
        for (suffix, value) in &self.unsigned_values {
            environment.mark_known_u64(format!("{path}{suffix}"), *value);
        }
    }
}

/// Environment maps are ordered by storage path, so a copy visits only the
/// source root's contiguous prefix rather than every fact in the state.
fn copy_retained<Value: Copy>(
    retained: &BTreeMap<String, Value>,
    source: &str,
    destination: &str,
    copied: &mut Vec<(String, Value)>,
) {
    copied.extend(
        retained
            .range(source.to_owned()..)
            .take_while(|(path, _)| path.starts_with(source))
            .filter_map(|(path, value)| {
                let suffix = path.strip_prefix(source)?;
                (suffix.is_empty() || suffix.starts_with('.') || suffix.starts_with('['))
                    .then(|| (format!("{destination}{suffix}"), *value))
            }),
    );
}
