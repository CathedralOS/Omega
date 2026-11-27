//! A checked branch can constrain the value later observed in private storage.
//! Provider declarations name formal positions. Receiver conditions name only
//! actual scalars, including the original selected entry's typed inputs.
use super::super::{
    facts::Facts,
    progress::ScalarFact,
    spin::{CellUpdate, Comparison, Operand, SpinCondition},
};
use crate::entry_values::TerminalScalarValue as Scalar;
use semantic_vocabulary::{Proposition, ScalarType, ValueId};
use std::cmp::Ordering;
use terminal_psi::OperationKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ScalarTest<T> {
    left: T,
    right: T,
    order: Ordering,
    positive: bool,
}

pub(super) type Selector = ScalarTest<Operand>;
pub(super) type Assumption = ScalarTest<ScalarFact>;

impl<T: Copy> ScalarTest<T> {
    pub(super) fn map<U>(self, mut bind: impl FnMut(T) -> Option<U>) -> Option<ScalarTest<U>> {
        Some(ScalarTest {
            left: bind(self.left)?,
            right: bind(self.right)?,
            order: self.order,
            positive: self.positive,
        })
    }

    pub(super) fn selected(mut self, answer: bool) -> Self {
        self.positive = self.positive == answer;
        self
    }

    fn equals(left: T, right: T) -> Self {
        Self {
            left,
            right,
            order: Ordering::Equal,
            positive: true,
        }
    }
}

impl Selector {
    pub(super) fn canonical(mut self) -> (Self, bool) {
        let inverted = !self.positive;
        self.positive = true;
        let reverse = match (self.left, self.right) {
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
            std::mem::swap(&mut self.left, &mut self.right);
            self.order = self.order.reverse();
        }
        (self, inverted)
    }

    pub(super) fn map_operands(
        &mut self,
        apply: &mut impl FnMut(Operand) -> Option<Operand>,
    ) -> Option<()> {
        *self = self.map(apply)?;
        Some(())
    }

    pub(super) fn actual_inputs(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
    ) -> Option<Assumption> {
        self.map(|operand| operand.binding(scalar))
    }

    pub(super) fn answer(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
        requirements: &[Proposition],
    ) -> Option<bool> {
        if self.left == self.right {
            return Some((self.order == Ordering::Equal) == self.positive);
        }
        self.actual_inputs(scalar)?.answer(requirements)
    }
}

impl Assumption {
    pub(super) fn boolean(value: ScalarFact) -> Option<Self> {
        (value.ty() == ScalarType::Boolean).then_some(Self::equals(
            value,
            ScalarFact::Literal(Scalar::Boolean(true)),
        ))
    }

    pub(super) fn of_operation(
        kind: &OperationKind,
        values: &Facts<ValueId, ScalarFact>,
        predicates: &Facts<ValueId, Self>,
    ) -> Option<Self> {
        if let OperationKind::BooleanNot { operand } = kind {
            return predicates
                .get(operand)
                .copied()
                .or_else(|| Self::boolean(*values.get(operand)?))
                .map(|test| test.selected(false));
        }
        let (left, right, order, positive) = Comparison::operands(kind)?;
        Some(Self {
            left: *values.get(&left)?,
            right: *values.get(&right)?,
            order,
            positive,
        })
    }

    pub(super) fn mentions(self, fact: ScalarFact) -> bool {
        self.left == fact || self.right == fact
    }

    pub(super) fn proposition(self) -> Option<Proposition> {
        let (left, right) = (self.left.term()?, self.right.term()?);
        Some(match (self.order, self.positive) {
            (Ordering::Equal, true) => Proposition::Equal(left, right),
            (Ordering::Equal, false) => Proposition::Implication {
                premise: Box::new(Proposition::Equal(left, right)),
                conclusion: Box::new(Proposition::Falsehood),
            },
            (Ordering::Less, true) => Proposition::LessThan(left, right),
            (Ordering::Less, false) => Proposition::LessOrEqual(right, left),
            (Ordering::Greater, true) => Proposition::LessThan(right, left),
            (Ordering::Greater, false) => Proposition::LessOrEqual(left, right),
        })
    }

    fn answer(self, requirements: &[Proposition]) -> Option<bool> {
        if self.left == self.right {
            return Some((self.order == Ordering::Equal) == self.positive);
        }
        let (left, pivot, order) = if matches!(self.left, ScalarFact::Literal(_)) {
            (self.right, self.left, self.order.reverse())
        } else {
            (self.left, self.right, self.order)
        };
        let ScalarFact::Literal(pivot) = pivot else {
            return super::super::progress::comparison::decided(
                left,
                pivot,
                order,
                self.positive,
                requirements,
            );
        };
        let mut test = SpinCondition {
            comparison: Comparison {
                pivot: Operand::Literal(pivot),
                order,
                mask: None,
            },
            positive: self.positive,
        };
        let impossible = |test: SpinCondition| {
            test.failure(Some(left), None, Some(CellUpdate::Observe), requirements)
                .is_none()
        };
        if impossible(test) {
            return Some(false);
        }
        test.positive = !test.positive;
        impossible(test).then_some(true)
    }
}
