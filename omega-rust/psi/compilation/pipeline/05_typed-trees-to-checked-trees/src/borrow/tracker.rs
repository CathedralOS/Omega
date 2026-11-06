use arena::Handle;
use checked_trees::name::Identifier;
use symbols::SymbolHandle;

use super::accesses::BorrowAccessPlace;

pub(crate) use checked_trees::BorrowLoanOwnerSegment as BorrowOwnerSegment;

/// Nearest borrowing occurrence whose permission a descriptor transports.
/// A copy carries this association; it does not become another borrowing parent.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct RetainedBorrowSource {
    pub(super) loan: Handle<checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct StateLoanTracker {
    pub(super) handle: Handle<checked_trees::BorrowLoanFact>,
    pub(super) owner_symbol: SymbolHandle,
    pub(super) storage_owner: facts::PlaceRoot,
    pub(super) owner_name: Identifier,
    pub(super) kind: checked_trees::BorrowAccessKind,
    pub(super) lineage: checked_trees::BorrowLoanLineage,
    /// A result root supplies no borrowing ancestry on its own. An exact
    /// descriptor transfer may still carry an earlier borrowing occurrence.
    pub(super) formation: super::loans::BorrowFormation,
    /// Value-relative result path at formation. A descriptor loaded from a
    /// field keeps that field as its source home, while the produced value's
    /// own reference is at the empty path.
    pub(super) formation_expression: checked_trees::expression::ExpressionHandle,
    pub(super) formation_owner_path: Option<Vec<BorrowOwnerSegment>>,
    pub(super) permission_sources: Vec<RetainedBorrowSource>,
    /// Projection within a borrow-carrying owner that holds this loan. Fixed
    /// array literal positions retain their ordinal; a dynamic later index
    /// still conservatively matches every ordinal.
    pub(super) owner_path: Vec<BorrowOwnerSegment>,
    pub(super) place: BorrowAccessPlace,
    pub(super) representation_address: Option<BorrowAccessPlace>,
    pub(super) retained_operand: Option<checked_trees::BorrowRetainedOperand>,
}
