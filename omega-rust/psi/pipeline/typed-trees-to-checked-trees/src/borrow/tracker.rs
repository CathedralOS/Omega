use crate::context::*;

use super::accesses::BorrowAccessPlace;

pub(crate) use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

#[derive(Clone)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) kind: checked_trees::BorrowAccessKind,
    pub(super) lineage: checked_trees::BorrowLoanLineage,
    /// Projection within a borrow-carrying owner that holds this loan. Fixed
    /// array literal positions retain their ordinal; a dynamic later index
    /// still conservatively matches every ordinal.
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: BorrowAccessPlace,
}
