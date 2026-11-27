//! A retry condition compares one observed word with a typed literal.
//! Reversing the operands or negating the branch preserves that comparison.

use super::Scalar;
use crate::synchronization::progress::ScalarFact;
use semantic_vocabulary::ScalarType;
use std::cmp::Ordering;

/// Scalar side of a retry comparison, in the selected machine's signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum Operand {
    Literal(Scalar),
    Parameter {
        position: usize,
        scalar_type: ScalarType,
    },
}

impl Operand {
    pub(in super::super) fn literal(self) -> Option<Scalar> {
        match self {
            Self::Literal(value) => Some(value),
            Self::Parameter { .. } => None,
        }
    }

    pub(super) fn intersect(self, prior: Option<Self>) -> Option<Self> {
        match prior {
            None => Some(self),
            Some(old) if old == self => Some(self),
            Some(old) => Some(Self::Literal(masked(
                self.literal()?,
                Some(old.literal()?),
            )?)),
        }
    }

    pub(in super::super) fn scalar_type(self) -> ScalarType {
        match self {
            Self::Literal(value) => value.scalar_type(),
            Self::Parameter { scalar_type, .. } => scalar_type,
        }
    }

    pub(super) fn formals<W>(
        machine: &terminal_psi::TerminalMachine,
        word: impl Fn(Self) -> W,
    ) -> super::Facts<semantic_vocabulary::ValueId, W> {
        machine
            .parameters
            .iter()
            .enumerate()
            .map(|(position, formal)| {
                (
                    formal.id,
                    word(Self::Parameter {
                        position,
                        scalar_type: formal.scalar_type,
                    }),
                )
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) struct Comparison {
    pub(in super::super) pivot: Operand,
    pub(in super::super) order: Ordering,
    pub(in super::super) mask: Option<Operand>,
}

/// An observed word can be restricted to the bits selected by one or more
/// literal masks. Bits outside that intersection have no bearing on its test.
pub(super) fn masked(value: Scalar, mask: Option<Scalar>) -> Option<Scalar> {
    match mask {
        None => Some(value),
        Some(mask) => crate::atomic_read_modify_write(
            terminal_psi::AtomicReadModifyWrite::FetchAnd,
            value,
            mask,
        ),
    }
}

impl Comparison {
    pub(super) fn operands(
        kind: &terminal_psi::OperationKind,
    ) -> Option<(
        semantic_vocabulary::ValueId,
        semantic_vocabulary::ValueId,
        Ordering,
        bool,
    )> {
        use terminal_psi::OperationKind as Op;
        match *kind {
            Op::IntegerEqual { left, right } | Op::BooleanEqual { left, right } => {
                Some((left, right, Ordering::Equal, true))
            }
            Op::IntegerLessThan { left, right } => Some((left, right, Ordering::Less, true)),
            Op::IntegerLessOrEqual { left, right } => Some((left, right, Ordering::Greater, false)),
            _ => None,
        }
    }

    pub(super) fn substitute(
        &mut self,
        resolve: impl Fn(Operand) -> Option<Operand>,
    ) -> Option<()> {
        self.pivot = resolve(self.pivot)?;
        if let Some(mask) = self.mask {
            self.mask = Some(resolve(mask)?);
        }
        Some(())
    }

    pub(super) fn equal(pivot: Scalar) -> Self {
        Self {
            pivot: Operand::Literal(pivot),
            order: Ordering::Equal,
            mask: None,
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
        let actual = ScalarFact::Literal(match self.comparison.mask {
            Some(mask) => masked(observed, Some(mask.literal()?))?,
            None => observed,
        })
        .known_integer();
        let pivot = ScalarFact::Literal(self.comparison.pivot.literal()?).known_integer();
        let ((ty, actual), (expected, pivot)) = actual.zip(pivot)?;
        (ty == expected).then(|| self.accepts(actual.cmp(&pivot)))
    }

    pub(in super::super) fn accepts(self, order: Ordering) -> bool {
        (self.comparison.order == order) == self.positive
    }

    /// Between two distinct pivots a value occupies at most five ordered
    /// regions. Treating the open interval as inhabited is conservative for
    /// adjacent integers and avoids overflowing when either pivot is an extreme.
    fn disjoint(self, other: Self) -> bool {
        use Ordering::{Equal, Greater, Less};
        let Some(((ty, left), (other_ty, right))) = self
            .comparison
            .pivot
            .literal()
            .and_then(|p| ScalarFact::Literal(p).known_integer())
            .zip(
                other
                    .comparison
                    .pivot
                    .literal()
                    .and_then(|p| ScalarFact::Literal(p).known_integer()),
            )
        else {
            return false;
        };
        if ty != other_ty || self.comparison.mask != other.comparison.mask {
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
                        comparison: Comparison {
                            pivot: Operand::Literal(pivot),
                            order,
                            mask: None,
                        },
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
        let Operand::Literal(Scalar::Integer { scalar_type, .. }) = self.comparison.pivot else {
            return false;
        };
        if self.comparison.order == Ordering::Equal || self.comparison.mask.is_some() {
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
        // All values with those selected bits pass the same equality test.
        if self.comparison.mask.is_some() {
            return None;
        }
        match (
            self.comparison.order,
            self.positive,
            self.comparison.pivot.literal()?,
        ) {
            (Ordering::Equal, true, pivot) => Some(pivot),
            (Ordering::Equal, false, Scalar::Boolean(bit)) => Some(Scalar::Boolean(!bit)),
            _ => None,
        }
    }
}
