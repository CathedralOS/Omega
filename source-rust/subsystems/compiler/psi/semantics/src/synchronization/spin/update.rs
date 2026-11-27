//! Transfers for a retrying atomic operation, before and after call-site substitution.

use super::{Scalar, condition};
use crate::synchronization::progress::ScalarFact;

/// These transfers are idempotent. Once a private retry observes the result
/// of its own update, all later iterations see those same bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in super::super) enum CellUpdate<V = Scalar> {
    Observe,
    Replace(V),
    BitwiseAnd(V),
    BitwiseOr(V),
    CompareExchange { expected: V, replacement: V },
}

impl CellUpdate {
    pub(in super::super) fn apply(self, prior: Scalar) -> Option<Scalar> {
        Some(match self {
            Self::Observe => prior,
            Self::Replace(value) => value,
            Self::BitwiseAnd(mask) => condition::masked(prior, Some(mask))?,
            Self::BitwiseOr(mask) => crate::atomic_read_modify_write(
                terminal_psi::AtomicReadModifyWrite::FetchOr,
                prior,
                mask,
            )?,
            Self::CompareExchange {
                expected,
                replacement,
            } => {
                if prior == expected {
                    replacement
                } else {
                    prior
                }
            }
        })
    }
}

impl<V> CellUpdate<V> {
    /// Substitute all operands together; a missing binding cannot become evidence.
    pub(in super::super) fn try_map<U>(
        self,
        mut take: impl FnMut(V) -> Option<U>,
    ) -> Option<CellUpdate<U>> {
        Some(match self {
            Self::Observe => CellUpdate::Observe,
            Self::Replace(value) => CellUpdate::Replace(take(value)?),
            Self::BitwiseAnd(mask) => CellUpdate::BitwiseAnd(take(mask)?),
            Self::BitwiseOr(mask) => CellUpdate::BitwiseOr(take(mask)?),
            Self::CompareExchange {
                expected,
                replacement,
            } => CellUpdate::CompareExchange {
                expected: take(expected)?,
                replacement: take(replacement)?,
            },
        })
    }
}

impl CellUpdate<ScalarFact> {
    pub(in super::super) fn next(self, prior: ScalarFact) -> Option<ScalarFact> {
        match self {
            Self::Observe => Some(prior),
            Self::Replace(value) => Some(value),
            Self::CompareExchange {
                expected,
                replacement,
            } if prior == expected => Some(replacement),
            Self::CompareExchange {
                expected,
                replacement,
            } if expected == replacement || prior == replacement => Some(prior),
            Self::CompareExchange {
                expected: ScalarFact::Literal(_),
                ..
            } if prior.literal().is_some() => Some(prior),
            Self::BitwiseAnd(operand) | Self::BitwiseOr(operand) if operand == prior => Some(prior),
            _ => self
                .try_map(ScalarFact::literal)?
                .apply(prior.literal()?)
                .map(ScalarFact::Literal),
        }
    }
}
