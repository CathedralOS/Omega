//! A checked branch can constrain the value later observed in private storage.
//! Provider declarations name formal positions. Receiver conditions name only
//! actual scalars, including the original selected entry's typed inputs.
use super::ScalarFact;
use super::comparison::Evidence;
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::{
    facts::BranchFacts,
    spin::{CellUpdate, Comparison, Operand, SpinCondition},
};
use semantic_vocabulary::{Proposition, ScalarType, ValueId};
use std::cmp::Ordering;
use terminal_psi::OperationKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::synchronization) struct ScalarTest<T> {
    left: T,
    right: T,
    order: Ordering,
    positive: bool,
}

pub(in crate::synchronization) type Selector = ScalarTest<Operand>;
pub(in crate::synchronization) type Assumption = ScalarTest<ScalarFact>;

impl<T: Copy> ScalarTest<T> {
    pub(in crate::synchronization) fn map<U>(
        self,
        mut bind: impl FnMut(T) -> Option<U>,
    ) -> Option<ScalarTest<U>> {
        Some(ScalarTest {
            left: bind(self.left)?,
            right: bind(self.right)?,
            order: self.order,
            positive: self.positive,
        })
    }

    pub(in crate::synchronization) fn selected(mut self, answer: bool) -> Self {
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
    pub(in crate::synchronization) fn canonical(mut self) -> (Self, bool) {
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

    pub(in crate::synchronization) fn map_operands(
        &mut self,
        apply: &mut impl FnMut(Operand) -> Option<Operand>,
    ) -> Option<()> {
        *self = self.map(apply)?;
        Some(())
    }

    pub(in crate::synchronization) fn actual_inputs(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
    ) -> Option<Assumption> {
        self.map(|operand| operand.binding(scalar))
    }

    pub(in crate::synchronization) fn answer(
        self,
        scalar: &impl Fn(usize) -> Option<ScalarFact>,
        requirements: &Evidence<'_>,
    ) -> Option<bool> {
        if self.left == self.right {
            return Some((self.order == Ordering::Equal) == self.positive);
        }
        self.actual_inputs(scalar)?.answer(requirements)
    }
}

impl Assumption {
    pub(in crate::synchronization) fn boolean(value: ScalarFact) -> Option<Self> {
        (value.ty() == ScalarType::Boolean).then_some(Self::equals(
            value,
            ScalarFact::Literal(Scalar::Boolean(true)),
        ))
    }

    pub(in crate::synchronization) fn of_operation(
        kind: &OperationKind,
        values: &BranchFacts<ValueId, ScalarFact>,
        predicates: &BranchFacts<ValueId, Self>,
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

    pub(super) fn permits(&self, left: ScalarFact, right: ScalarFact, candidate: Ordering) -> bool {
        let actual = if self.left == left && self.right == right {
            candidate
        } else if self.left == right && self.right == left {
            candidate.reverse()
        } else {
            return true;
        };
        (actual == self.order) == self.positive
    }

    pub(in crate::synchronization) fn mentions(self, fact: ScalarFact) -> bool {
        self.left == fact || self.right == fact
    }

    pub(in crate::synchronization) fn proposition(self) -> Option<Proposition> {
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

    fn answer(self, requirements: &Evidence<'_>) -> Option<bool> {
        if self.left == self.right {
            return Some((self.order == Ordering::Equal) == self.positive);
        }
        let (left, pivot, order) = if matches!(self.left, ScalarFact::Literal(_)) {
            (self.right, self.left, self.order.reverse())
        } else {
            (self.left, self.right, self.order)
        };
        let ScalarFact::Literal(pivot) = pivot else {
            return super::comparison::decided(left, pivot, order, self.positive, requirements);
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
