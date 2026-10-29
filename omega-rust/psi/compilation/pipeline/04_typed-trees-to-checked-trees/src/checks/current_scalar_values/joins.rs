//! A completed scalar dispatch owns one value, independent of which arm won.
//! Retain only bounds established for every captured arm. Anonymous integer
//! literals need no fabricated runtime type to contribute mathematical bounds;
//! source checking independently lands the completed result in its carrier.

use super::CurrentScalarValues;
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, TableMatchExpression};
use typed_trees::{machine::Machine, state::State};
use validation::ScopedArithmeticValue;

impl CurrentScalarValues<'_> {
    pub(super) fn capture_join(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        dispatch: &TableMatchExpression,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return None;
        }
        let mut captured = arms
            .iter()
            .map(|arm| self.capture_at_depth(program, machine, state, arm.value, depth + 1))
            .collect::<Option<Vec<_>>>()?;
        if captured.iter().skip(1).all(|value| {
            value == &captured[0]
                || self.proves_values_relation(
                    program,
                    machine,
                    BinaryOperator::Equal,
                    value,
                    &captured[0],
                )
        }) {
            return Some(captured.swap_remove(0));
        }
        let bounds = captured
            .iter()
            .map(|value| {
                validation::scoped_arithmetic_value_bounds(
                    program,
                    machine,
                    &self.hypotheses,
                    value,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let minimum = bounds
            .iter()
            .map(|bound| bound.low.clone())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .min()?;
        let maximum = bounds
            .iter()
            .map(|bound| bound.high.clone())
            .collect::<Option<Vec<_>>>()?
            .into_iter()
            .max()?;
        let unsigned = minimum >= BigInt::zero();
        Some(ScopedArithmeticValue::BoundedInteger {
            value: Box::new(ScopedArithmeticValue::Atom {
                identity: format!("state:{:?}:join-result:{expression:?}", state.symbol),
                unsigned,
            }),
            minimum: Some(minimum),
            maximum: Some(maximum),
        })
    }
}
