use crate::context::*;

use super::accesses::BorrowAccessPlace;

#[derive(Debug, Clone, Copy)]
pub(super) enum BorrowOwnerSegment {
    Field(SymbolHandle),
    AnyIndex,
}

#[derive(Clone)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<omega_checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    /// Projection within a borrow-carrying owner that holds this loan. Array
    /// literal positions use `AnyIndex` because the initializer has no stable
    /// index expression handle to reuse at later access sites.
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: BorrowAccessPlace,
}
