use crate::checked_trees::name::Identifier;
use arena::Handle;
use symbols::SymbolHandle;

use super::accesses::BorrowAccessPlace;

pub(crate) use crate::checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

#[derive(Clone)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<crate::checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) kind: crate::checked_trees::BorrowAccessKind,
    pub(super) lineage: crate::checked_trees::BorrowLoanLineage,
    /// The retained root was captured from a call result rather than a borrow
    /// expression. It is not borrow ancestry: children formed through it must
    /// not claim `Reborrow` lineage against this loan.
    pub(super) call_result: bool,
    /// Projection within a borrow-carrying owner that holds this loan. Fixed
    /// array literal positions retain their ordinal; a dynamic later index
    /// still conservatively matches every ordinal.
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: BorrowAccessPlace,
}
