//! A branch in a provider names the same scalar comparison as a retry. Retain
//! both operands so the receiver can substitute its actuals and entry premises.
use super::{CellUpdate, Operand, Scalar, ScalarFact, SpinCondition};
use crate::synchronization::{facts::Facts, spin::Comparison};
use semantic_vocabulary::{Proposition, ScalarType, ValueId};
use std::cmp::Ordering;
use terminal_psi::OperationKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selector {
    value: Operand,
    test: SpinCondition,
}

impl Selector {
    /// Give a test and its negation the same identity. Reversing the
    /// operands also reverses their order comparison.
    pub(super) fn canonical(mut self) -> (Self, bool) {
        let inverted = !self.test.positive;
        self.test.positive = true;
        let pivot = self.test.comparison.pivot;
        let reverse = match (self.value, pivot) {
            (Operand::Literal(_), Operand::Parameter { .. }) => true,
            (
                Operand::Parameter { position: left, .. },
                Operand::Parameter {
                    position: right, ..
                },
            ) => left > right,
            _ => false,
        };
        if reverse {
            self.test.comparison.pivot = self.value;
            self.value = pivot;
            self.test.comparison.order = self.test.comparison.order.reverse();
        }
        (self, inverted)
    }

    pub(super) fn boolean(value: Operand) -> Option<Self> {
        (value.scalar_type() == ScalarType::Boolean).then_some(Self {
            value,
            test: SpinCondition {
                comparison: Comparison::equal(Scalar::Boolean(true)),
                positive: true,
            },
        })
    }

    pub(super) fn of_operation(
        kind: &OperationKind,
        operand: impl Fn(ValueId) -> Option<Operand>,
        predicates: &Facts<ValueId, Self>,
    ) -> Option<Self> {
        if let OperationKind::BooleanNot { operand: source } = kind {
            let mut selector = operand(*source)
                .and_then(Self::boolean)
                .or_else(|| predicates.get(source).copied())?;
            selector.test.positive = !selector.test.positive;
            return Some(selector);
        }
        let (left, right, mut order, positive) = Comparison::operands(kind)?;
        let mut value = operand(left)?;
        let mut pivot = operand(right)?;
        if value.literal().is_some() && pivot.literal().is_none() {
            std::mem::swap(&mut value, &mut pivot);
            order = order.reverse();
        }
        Some(Self {
            value,
            test: SpinCondition {
                comparison: Comparison {
                    pivot,
                    order,
                    mask: None,
                },
                positive,
            },
        })
    }

    pub(super) fn map_operands(
        &mut self,
        apply: &mut impl FnMut(Operand) -> Option<Operand>,
    ) -> Option<()> {
        self.value = apply(self.value)?;
        self.test.comparison.pivot = apply(self.test.comparison.pivot)?;
        Some(())
    }

    pub(super) fn actual_inputs(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
    ) -> Option<(ScalarFact, ScalarFact, Ordering, bool)> {
        Some((
            self.value.binding(scalar)?,
            self.test.comparison.pivot.binding(scalar)?,
            self.test.comparison.order,
            self.test.positive,
        ))
    }

    pub(super) fn answer(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
        requirements: &[Proposition],
    ) -> Option<bool> {
        if self.value == self.test.comparison.pivot {
            return Some(self.test.accepts(Ordering::Equal));
        }
        let (mut test, pivot) = self.test.with_actuals(|operand| operand.binding(scalar));
        let actual = self.value.binding(scalar);
        let impossible = |condition: SpinCondition| {
            condition
                .failure(actual, pivot, Some(CellUpdate::Observe), requirements)
                .is_none()
        };
        if impossible(test) {
            return Some(false);
        }
        test.positive = !test.positive;
        impossible(test).then_some(true)
    }
}
