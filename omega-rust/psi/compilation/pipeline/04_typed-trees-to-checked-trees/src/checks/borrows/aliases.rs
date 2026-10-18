//! Local reference operands rejoin retained storage before overlap comparison.
//! A captured place does not grant ancestry: only receivers with independently
//! retained lineage may exempt their own parent loans from interference.
use arena::{Handle, HandleSpan};
use checked_trees::{BorrowFacts, BorrowLoanFact, BorrowLoanLineage, StateBorrowFact};
use checked_trees::{CapturedPlace, CheckFacts, FlowStateFact};
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

pub(in crate::checks::borrows) struct ResolvedAlias {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) lineage: Vec<Handle<BorrowLoanFact>>,
}

impl ResolvedAlias {
    /// Lineage names descriptor occurrences, each of which may retain several
    /// alternative origins. Comparing one origin with another origin of that
    /// same descriptor must not treat them as simultaneous exclusive loans.
    /// The producer groups a completed descriptor by storage, statement and
    /// leaf path; match arms may have distinct formation expressions. A later
    /// replacement, another leaf or a child descriptor is a different occurrence.
    pub(in crate::checks::borrows) fn retains_loan(
        &self,
        borrow: &BorrowFacts,
        handle: Handle<BorrowLoanFact>,
    ) -> bool {
        self.lineage
            .iter()
            .any(|ancestor| same_descriptor(borrow, *ancestor, handle))
    }
}

/// Alternatives belong to an exact descriptor occurrence, never merely to
/// equal backing geometry. Wildcard paths may describe coexisting slots.
pub(in crate::checks::borrows) fn same_descriptor(
    borrow: &BorrowFacts,
    left: Handle<BorrowLoanFact>,
    right: Handle<BorrowLoanFact>,
) -> bool {
    if left == right {
        return left.is_valid();
    }
    if !borrow.loans.is_valid(left) || !borrow.loans.is_valid(right) {
        return false;
    }
    let left = borrow.loans.get(left);
    let right = borrow.loans.get(right);
    let path = borrow.loan_owner_path(right);
    right.owner_symbol.is_valid()
        && right.storage_owner.is_valid()
        && left.owner_symbol == right.owner_symbol
        && left.storage_owner == right.storage_owner
        && left.statement_index == right.statement_index
        && borrow.loan_owner_path(left) == path
        && !path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
}

pub(in crate::checks::borrows) struct ResolvedPlace {
    pub(in crate::checks::borrows) place: CapturedPlace,
    pub(in crate::checks::borrows) local_loan: Handle<BorrowLoanFact>,
}

pub(in crate::checks::borrows) fn resolve_place(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<ResolvedPlace> {
    let mut places = resolve_places(program, facts, state_flow, entry_constraints, place)?;
    (places.len() == 1).then(|| places.remove(0))
}

pub(in crate::checks::borrows) fn resolve_places(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    entry_constraints: HandleSpan<checked_trees::FlowConstraintRef>,
    place: CapturedPlace,
) -> Option<Vec<ResolvedPlace>> {
    let state = program.state_by_symbol(state_flow.state_symbol)?;
    let reference_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local)
                if facts::PlaceRoot::Symbol(local.symbol) == place.root
                    && matches!(program.type_reference_table.type_reference(local.type_reference),
                        TypeReferenceNode::Reference { .. }))
        });
    if !reference_local {
        return Some(vec![ResolvedPlace {
            place,
            local_loan: Handle::invalid(),
        }]);
    }

    let borrow = &facts.borrow;
    let state_borrow = borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == state_flow.machine_symbol
            && state.state_symbol == state_flow.state_symbol)
            .then_some(state)
    })?;
    let candidates = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .filter(|handle| {
            borrow.state_owns_loan(state_borrow, *handle)
                && borrow.loans.get(*handle).storage_owner == place.root
        });
    let places: Vec<_> = candidates
        .map(|handle| {
            let loan = borrow.loans.get(handle);
            if !loan.owner_path.is_empty() || !loan.root.is_valid() {
                return None;
            }
            let mut segments = borrow.loan_segments(loan).to_vec();
            // The selected descriptor retains every possible backing, not its
            // private storage slot. Formation already captured each exact root.
            if loan.lineage != BorrowLoanLineage::LifetimeDependency {
                segments.extend_from_slice(&place.segments);
            }
            Some(ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    root: loan.root,
                    segments,
                },
                local_loan: handle,
            })
        })
        .collect::<Option<_>>()?;
    (!places.is_empty()).then_some(places)
}

pub(in crate::checks::borrows) fn retained_lineage(
    borrow: &BorrowFacts,
    state: &StateBorrowFact,
    handle: Handle<BorrowLoanFact>,
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
                BorrowLoanLineage::Reborrow { parent_loan } => std::slice::from_ref(parent_loan),
                BorrowLoanLineage::UnretainedDerived => {
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
            }
            BorrowLoanLineage::UnretainedDerived => {
                let suppliers = borrow.loan_suppliers.span(loan.supplying_loans)?;
                if suppliers.is_empty() {
                    return None;
                }
                for supplier in suppliers {
                    if !borrow.state_owns_loan(state, *supplier) {
                        return None;
                    }
                    let source = borrow.loans.get(*supplier);
                    if loan.source_owner_symbol != source.owner_symbol
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
