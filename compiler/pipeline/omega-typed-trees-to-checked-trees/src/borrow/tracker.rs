use crate::context::*;

use super::accesses::BorrowAccessPlace;

#[derive(Clone)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<omega_checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) owner_name: Identifier,
    pub(super) place: BorrowAccessPlace,
}
