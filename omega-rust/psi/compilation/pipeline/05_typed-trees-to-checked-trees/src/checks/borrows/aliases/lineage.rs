//! Follow the recorded supplier history of a saved reference. Formation may
//! retain lifetime obligations; ordinary access still requires a descriptor
//! route. Every supplier must belong to the same state and grant the carried
//! access. Joined routes retain only ancestors present on every route.

use super::same_descriptor;
use arena::Handle;
use checked_trees::{
    BorrowFacts, BorrowLoanFact, BorrowLoanLineage, BorrowLoanOwnerSegment, StateBorrowFact,
};

/// Forming a lifetime debt retains an already supplied input; it does not
/// borrow that input again. Authenticate every supplier before admitting its
/// formation alongside a common ancestor. This query is only for formation:
/// ordinary descriptor access still rejects LifetimeDependency as a source.
pub(in crate::checks::borrows) fn formation_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    descriptor_lineage(borrow, state, handle, LineagePurpose::Formation)
}

/// The exact occurrences a descriptor transfer transports. Each supplier is
/// owned by this state, is read through the transfer's source owner and
/// grants at least the transported access, so the transfer adds no borrow,
/// owner or access of its own. A transfer naming no supplier has no lineage.
pub(in crate::checks) fn transport_suppliers<'borrow>(
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
fn retained_input_terminal(
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

pub(in crate::checks::borrows) fn retained_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    descriptor_lineage(borrow, state, handle, LineagePurpose::Access)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineagePurpose {
    Access,
    Formation,
}

fn descriptor_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
    purpose: LineagePurpose,
) -> Option<Vec<Handle<BorrowLoanFact>>> {
    // A descriptor transfer transports its suppliers' established lineage,
    // without turning the copied carrier into a new reborrow parent. Traverse
    // exact historical identities only: backing coincidence grants nothing.
    // Every alternative must be complete; a partial supplier graph is unknown.
    // Equal-place alternatives may have different histories. Only ancestors
    // common to every supplier justify an access exemption after their join.
    let mut completed: Vec<(Handle<BorrowLoanFact>, Vec<Handle<BorrowLoanFact>>)> = Vec::new();
    let mut active = Vec::new();
    let mut pending = vec![(handle, false)];
    while let Some((handle, closing)) = pending.pop() {
        if closing {
            if active.pop() != Some(handle) {
                return None;
            }
            let loan = borrow.loans.get(handle);
            let parents = match &loan.lineage {
                BorrowLoanLineage::DirectRoot => &[][..],
                BorrowLoanLineage::SuppliedOutput => {
                    borrow.supplied_output_call(handle)?;
                    &[][..]
                }
                BorrowLoanLineage::Reborrow { parent_loan } => std::slice::from_ref(parent_loan),
                BorrowLoanLineage::UnretainedDerived => {
                    borrow.loan_suppliers.span(loan.supplying_loans)?
                }
                BorrowLoanLineage::LifetimeDependency if purpose == LineagePurpose::Formation => {
                    borrow.loan_suppliers.span(loan.supplying_loans)?
                }
                BorrowLoanLineage::LifetimeDependency => return None,
            };
            let mut common = Vec::new();
            for (position, parent) in parents.iter().enumerate() {
                let (_, ancestors) = completed
                    .iter()
                    .find(|(candidate, _)| candidate == parent)?;
                if position == 0 {
                    common.clone_from(ancestors);
                } else {
                    // Alternative origins have distinct loan rows but may
                    // belong to the same exact descriptor occurrence. Keep
                    // only occurrences present on every supplier path, not
                    // merely identical row handles or equal backing places.
                    common.retain(|ancestor| {
                        ancestors
                            .iter()
                            .any(|candidate| same_descriptor(borrow, *ancestor, *candidate))
                    });
                }
            }
            if matches!(loan.lineage, BorrowLoanLineage::Reborrow { .. }) {
                // A real borrow can be reached through saved descriptors.
                // Retain those exact forwarding occurrences as well as its
                // borrowing parent; call-input debts belong to that route even
                // when the new view selects a smaller physical projection.
                let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                let mut forwarded = Vec::new();
                for (position, supplier) in suppliers.iter().enumerate() {
                    let (_, ancestors) = completed
                        .iter()
                        .find(|(candidate, _)| candidate == supplier)?;
                    if position == 0 {
                        forwarded.clone_from(ancestors);
                    } else {
                        forwarded.retain(|ancestor| {
                            ancestors
                                .iter()
                                .any(|candidate| same_descriptor(borrow, *ancestor, *candidate))
                        });
                    }
                }
                for ancestor in forwarded {
                    crate::shared::collections::push_unique(&mut common, ancestor);
                }
            }
            common.push(handle);
            completed.push((handle, common));
            continue;
        }
        if completed.iter().any(|(candidate, _)| *candidate == handle) {
            continue;
        }
        if !borrow.state_owns_loan(state, handle) || active.contains(&handle) {
            return None;
        }
        active.push(handle);
        pending.push((handle, true));
        let loan = borrow.loans.get(handle);
        match loan.lineage {
            BorrowLoanLineage::DirectRoot => {}
            BorrowLoanLineage::SuppliedOutput => {
                borrow.supplied_output_call(handle)?;
            }
            BorrowLoanLineage::Reborrow { parent_loan } => {
                if !borrow.state_owns_loan(state, parent_loan) {
                    return None;
                }
                let parent = borrow.loans.get(parent_loan);
                if loan.source_owner_symbol != parent.owner_symbol
                    || parent.kind.direct_reborrow_effect(&loan.kind).is_none()
                {
                    return None;
                }
                pending.push((parent_loan, false));
                for supplier in borrow.loan_suppliers.span(loan.supplying_loans)? {
                    if !borrow.state_owns_loan(state, *supplier)
                        || borrow
                            .loans
                            .get(*supplier)
                            .kind
                            .direct_reborrow_effect(&loan.kind)
                            .is_none()
                    {
                        return None;
                    }
                    pending.push((*supplier, false));
                }
            }
            BorrowLoanLineage::UnretainedDerived => {
                if !retained_input_terminal(borrow, state, loan) {
                    for supplier in transport_suppliers(borrow, state, handle)? {
                        pending.push((*supplier, false));
                    }
                }
            }
            BorrowLoanLineage::LifetimeDependency if purpose == LineagePurpose::Formation => {
                let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                if suppliers.is_empty() || !loan.source_owner_symbol.is_valid() {
                    return None;
                }
                for supplier in suppliers {
                    if !borrow.state_owns_loan(state, *supplier) {
                        return None;
                    }
                    let source = borrow.loans.get(*supplier);
                    if source.owner_symbol != loan.source_owner_symbol
                        || source.kind.direct_reborrow_effect(&loan.kind).is_none()
                    {
                        return None;
                    }
                    pending.push((*supplier, false));
                }
            }
            BorrowLoanLineage::LifetimeDependency => return None,
        }
    }
    completed
        .into_iter()
        .find_map(|(candidate, lineage)| (candidate == handle).then_some(lineage))
}
