//! Closed operation family selected by the exact unary integer rule entrances.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IntegerUnaryKind {
    Widen,
    BitwiseNot,
}
