//! Replacing a reference slot must not end access held by an earlier copy.
//! Keep the original borrow held until all surviving copies finish. Lexical
//! slot retirement remains separate from the final holder's resource closure.

use arena::Handle;
use checked_trees::{
    BorrowLoanFact, CheckFacts, CheckedBorrowResourceRetention, FlowBorrowWeakeningFact,
};
use diagnostics::Diagnostic;

use super::lifecycle::{activation_boundary, exact_loan_lifecycle_rows, weakening_event_boundary};

#[derive(Default)]
struct TransferHolders {
    loan: Handle<BorrowLoanFact>,
    holders: Vec<Handle<BorrowLoanFact>>,
}

#[derive(Default)]
pub(super) struct LoanRetention {
    pub(super) loan: Handle<BorrowLoanFact>,
    pub(super) retention: CheckedBorrowResourceRetention,
}

/// Resolve each complete holder closure before installing resource lifetimes.
/// A holder must capture its supplier before that descriptor's lexical death;
/// a later view cannot retrospectively establish its own parent's permission.
pub(super) fn plan(facts: &mut CheckFacts) -> Result<Vec<LoanRetention>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    let mut holders_arena = arena::Arena::new();
    // Build direct forwarding edges once. Dense slots are lookup coordinates;
    // the complete generational handle still authenticates every row.
    let mut transfers: Vec<TransferHolders> = Vec::new();
    for (_, state) in facts.borrow.states.iter() {
        for (holder, _) in facts.borrow.state_loans(state) {
            let Some(suppliers) =
                crate::checks::borrows::transport_suppliers(&facts.borrow, state, holder)
            else {
                continue;
            };
            for supplier in suppliers {
                let position = supplier.arena_index() as usize;
                if transfers.len() <= position {
                    transfers.resize_with(position + 1, TransferHolders::default);
                }
                let entry = &mut transfers[position];
                entry.loan = *supplier;
                crate::shared::collections::push_unique(&mut entry.holders, holder);
            }
        }
    }
    for (_, state) in facts.borrow.states.iter() {
        let Some(flow_state) = facts
            .flow
            .control
            .state(state.machine_symbol, state.state_symbol)
        else {
            return Err(vec![Diagnostic::error(
                "retained borrow has no flow-state owner",
            )]);
        };
        for (loan, row) in facts.borrow.state_loans(state) {
            if !matches!(
                row.lineage,
                checked_trees::BorrowLoanLineage::DirectRoot
                    | checked_trees::BorrowLoanLineage::Reborrow { .. }
            ) {
                continue;
            }
            let Ok((_, lexical)) =
                exact_loan_lifecycle_rows(&facts.borrow, &facts.flow, flow_state, loan)
            else {
                // Unevaluated formations have no permission to retain. Resource
                // construction reports any required missing lifecycle rows.
                continue;
            };
            let mut holders = vec![loan];
            // Each holder's own end, used when it supplies a later copy.
            let mut holder_ends = vec![lexical];
            let mut cursor = 0;
            let mut final_weakening = lexical;
            while cursor < holders.len() {
                let supplier = holders[cursor];
                let supplier_end_handle = holder_ends[cursor];
                let supplier_end = facts
                    .flow
                    .borrow_lifetimes
                    .weakenings
                    .get(supplier_end_handle);
                cursor += 1;
                let Some(transferred) = transfers
                    .get(supplier.arena_index() as usize)
                    .filter(|row| row.loan == supplier)
                else {
                    continue;
                };
                for candidate in transferred.holders.iter().copied() {
                    if holders.contains(&candidate) {
                        continue;
                    }
                    // A descriptor a completed call installed carries custody
                    // without minting a local loan, so it has no lifecycle rows
                    // (reference_custody.rs). Its slot's own loan covers the
                    // slot; keep walking through it to its later copies, which
                    // live no longer than the supplier that reached it.
                    if facts
                        .borrow
                        .loans
                        .get(candidate)
                        .established_by_call
                        .is_valid()
                    {
                        holder_ends.push(supplier_end_handle);
                        holders.push(candidate);
                        continue;
                    }
                    let (activation, weakening) = exact_loan_lifecycle_rows(
                        &facts.borrow,
                        &facts.flow,
                        flow_state,
                        candidate,
                    )
                    .map_err(|error| vec![error])?;
                    let activation = facts.flow.borrow_lifetimes.activations.get(activation);
                    if activation_boundary(activation.source)?
                        >= weakening_event_boundary(supplier_end.source, supplier_end.reason)?
                    {
                        return Err(vec![Diagnostic::error(
                            "saved descriptor was not captured before its supplier retired",
                        )]);
                    }
                    let end = facts.flow.borrow_lifetimes.weakenings.get(weakening);
                    let previous = facts.flow.borrow_lifetimes.weakenings.get(final_weakening);
                    if weakening_event_boundary(end.source, end.reason)?
                        > weakening_event_boundary(previous.source, previous.reason)?
                    {
                        final_weakening = weakening;
                    }
                    holder_ends.push(weakening);
                    holders.push(candidate);
                }
            }
            let position = loan.arena_index() as usize;
            if rows.len() <= position {
                rows.resize_with(position + 1, LoanRetention::default);
            }
            rows[position] = LoanRetention {
                loan,
                retention: CheckedBorrowResourceRetention {
                    lexical_weakening: lexical,
                    holders: holders_arena.insert_many(holders),
                    final_weakening,
                },
            };
        }
    }
    facts.borrow.resource_holders = holders_arena;
    Ok(rows)
}

pub(super) fn get(
    rows: &[LoanRetention],
    loan: Handle<BorrowLoanFact>,
) -> Result<&CheckedBorrowResourceRetention, Vec<Diagnostic>> {
    rows.get(loan.arena_index() as usize)
        .filter(|row| row.loan == loan)
        .map(|row| &row.retention)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "borrow resource has no complete descriptor-holder closure",
            )]
        })
}

pub(super) fn end<'flow>(
    facts: &'flow checked_trees::FlowFacts,
    retention: &CheckedBorrowResourceRetention,
) -> &'flow FlowBorrowWeakeningFact {
    facts
        .borrow_lifetimes
        .weakenings
        .get(retention.final_weakening)
}
