//! A saved pointer keeps the loans that supplied its original permission.
//! Follow only recorded source occurrences, checking every alternative and
//! access ceiling. Equal addresses do not identify a supplying occurrence.

use arena::Handle;
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, StateBorrowFact,
};

/// The exact occurrences a descriptor transfer transports. Each supplier is
/// owned by this state, is read through the transfer's source owner and
/// grants at least the transported access, so the transfer adds no borrow,
/// owner or access of its own. A transfer naming no supplier has no lineage.
pub(crate) fn transport_suppliers<'borrow>(
    borrow: &'borrow BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
) -> Option<&'borrow [Handle<BorrowLoanFact>]> {
    let loan = borrow.loans.get(handle);
    if loan.lineage != BorrowLoanLineage::UnretainedDerived {
        return None;
    }
    let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
    let exact = !suppliers.is_empty()
        && suppliers.iter().all(|supplier| {
            let source = borrow.loans.get(*supplier);
            borrow.state_owns_loan(state, *supplier)
                && source.owner_symbol == loan.source_owner_symbol
                && source.kind.direct_reborrow_effect(&loan.kind).is_some()
        });
    exact.then_some(suppliers)
}

/// A moved input descriptor retains its entry identity without a local parent
/// loan. Recognize only that admitted fixed leaf, not another descriptor with
/// the same backing. Its lineage contains this occurrence alone; it supplies
/// neither a new entry permission nor a local borrowing resource.
pub(crate) fn retained_input_terminal(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    loan: &BorrowLoanFact,
) -> bool {
    if loan.lineage != BorrowLoanLineage::UnretainedDerived
        || !borrow
            .loan_suppliers
            .span(loan.supplying_loans)
            .is_some_and(<[_]>::is_empty)
        || !borrow.state_owns_incoming_reference(state, loan.incoming_reference)
        || !borrow
            .access_segments
            .span(loan.segments)
            .is_some_and(<[_]>::is_empty)
        || !borrow
            .access_segments
            .span(loan.incoming_owner_path)
            .is_some_and(<[_]>::is_empty)
    {
        return false;
    }
    let incoming = borrow.incoming_references.get(loan.incoming_reference);
    if !borrow
        .owner_segments
        .span(incoming.owner_path)
        .is_some_and(|path| !path.contains(&BorrowLoanOwnerSegment::DynamicIndex))
    {
        return false;
    }
    matches!(incoming.origin, checked_trees::IncomingReferenceOrigin::Parameter { symbol }
        if symbol.is_valid()
            && loan.source_owner_symbol == symbol
            && loan.root == incoming.place_root())
}
