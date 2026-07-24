use crate::context::*;

use super::accesses::BorrowAccessPlace;

#[derive(Debug, Clone, Copy)]
pub(crate) enum BorrowOwnerSegment {
    Field(SymbolHandle),
    FixedIndex(usize),
}

#[derive(Clone)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<omega_checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) kind: omega_checked_trees::BorrowAccessKind,
    /// Projection within a borrow-carrying owner that holds this loan. Fixed
    /// array literal positions retain their ordinal; a dynamic later index
    /// still conservatively matches every ordinal.
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: BorrowAccessPlace,
}
