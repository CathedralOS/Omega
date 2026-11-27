//! A retry condition compares one observed word with a typed literal.
//! Reversing the operands or negating the branch preserves that comparison.

use super::Scalar;
use crate::synchronization::progress::ScalarFact;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Comparison {
    pub(in super::super) pivot: Scalar,
    pub(in super::super) order: Ordering,
}

impl Comparison {
    pub(super) fn equal(pivot: Scalar) -> Self {
        Self {
            pivot,
            order: Ordering::Equal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct SpinCondition {
    pub(in super::super) comparison: Comparison,
    pub(in super::super) positive: bool,
}

impl SpinCondition {
    pub(in super::super) fn matches(self, observed: Scalar) -> Option<bool> {
        let actual = ScalarFact::Literal(observed).known_integer();
        let pivot = ScalarFact::Literal(self.comparison.pivot).known_integer();
        let ((ty, actual), (expected, pivot)) = actual.zip(pivot)?;
        (ty == expected).then(|| self.accepts(actual.cmp(&pivot)))
    }

    fn accepts(self, order: Ordering) -> bool {
        (self.comparison.order == order) == self.positive
    }

    /// Between two distinct pivots a value occupies at most five ordered
    /// regions. Treating the open interval as inhabited is conservative for
    /// adjacent integers and avoids overflowing when either pivot is an extreme.
    fn disjoint(self, other: Self) -> bool {
        use Ordering::{Equal, Greater, Less};
        let Some(((ty, left), (other_ty, right))) = ScalarFact::Literal(self.comparison.pivot)
            .known_integer()
            .zip(ScalarFact::Literal(other.comparison.pivot).known_integer())
        else {
            return false;
        };
        if ty != other_ty {
            return false;
        }
        match left.cmp(&right) {
            Equal => [Less, Equal, Greater]
                .into_iter()
                .all(|order| !(self.accepts(order) && other.accepts(order))),
            Less => [
                (Less, Less),
                (Equal, Less),
                (Greater, Less),
                (Greater, Equal),
                (Greater, Greater),
            ]
            .into_iter()
            .all(|(left, right)| !(self.accepts(left) && other.accepts(right))),
            Greater => other.disjoint(self),
        }
    }

    pub(in super::super) fn excluded_by(
        self,
        input: terminal_psi::ValueDeclaration,
        requires: &[semantic_vocabulary::Proposition],
    ) -> bool {
        use semantic_vocabulary::{Proposition, ScalarTerm};
        let mut pending: Vec<_> = requires.iter().collect();
        while let Some(clause) = pending.pop() {
            let (a, b, order, positive) = match clause {
                Proposition::Conjunction(parts) => {
                    pending.extend(parts);
                    continue;
                }
                Proposition::LessThan(a, b) => (a, b, Ordering::Less, true),
                Proposition::LessOrEqual(a, b) => (a, b, Ordering::Greater, false),
                Proposition::Equal(a, b) => (a, b, Ordering::Equal, true),
                _ => continue,
            };
            for (subject, bound, order) in [(a, b, order), (b, a, order.reverse())] {
                if !matches!(subject, ScalarTerm::Value { id, scalar_type } if *id == input.id && *scalar_type == input.scalar_type)
                {
                    continue;
                }
                if let Some((ty, raw)) =
                    crate::inhabitation::known_scalar_value_with(bound, |_| None)
                    && let Some(ScalarFact::Literal(pivot)) = ScalarFact::from_integer(ty, raw)
                    && self.disjoint(Self {
                        comparison: Comparison { pivot, order },
                        positive,
                    })
                {
                    return true;
                }
            }
        }
        false
    }

    /// A one-sided integer comparison is monotone across the entire
    /// representable domain. If neither end blocks, no resident word can.
    pub(in super::super) fn excludes_every_word(self) -> bool {
        let Scalar::Integer { scalar_type, .. } = self.comparison.pivot else {
            return false;
        };
        if self.comparison.order == Ordering::Equal {
            return false;
        }
        let (Some(lower), Some(upper)) = (scalar_type.minimum_value(), scalar_type.maximum_value())
        else {
            return false;
        };
        [lower, upper]
            .into_iter()
            .all(|value| self.matches(Scalar::Integer { scalar_type, value }) == Some(false))
    }

    pub(in super::super) fn sole_blocked_value(self) -> Option<Scalar> {
        match (self.comparison.order, self.positive, self.comparison.pivot) {
            (Ordering::Equal, true, pivot) => Some(pivot),
            (Ordering::Equal, false, Scalar::Boolean(bit)) => Some(Scalar::Boolean(!bit)),
            _ => None,
        }
    }
}
