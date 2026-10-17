//! Selected descriptors have a receiving carrier and anonymous complement
//! slots. Closing the complete receiving leaf cohort also closes that leaf
//! in its unobservable complement; scalar/root custody keeps its normal death.
//! Exact historical transport, not shared spelling or backing, joins the
//! original suppliers. This correspondence precedes parent retirement and is
//! retained for emission rather than rediscovered from runtime source shapes.

use super::{BorrowLoanFact, CheckedUnitEffectOperationPlan, Retirement};
use arena::{Handle, HandleSpan};
use checked_trees::CheckedUnitSelectedReferenceRetirementPlan;

/// Transport records include copies, so ancestry alone cannot dispose a
/// supplier. Only the exact current candidate occurrence owned by this join
/// dies here; an older supplier still needs its own definite move or disposal.
pub(super) fn disposes_candidate(
    retirement: &Retirement<'_>,
    handle: Handle<BorrowLoanFact>,
    complement: &CheckedUnitSelectedReferenceRetirementPlan,
) -> bool {
    let receipt = retirement
        .facts
        .flow
        .ownership
        .owned_selections
        .get(complement.selection);
    let loan = retirement.facts.borrow.loans.get(handle);
    if !retirement
        .facts
        .flow
        .ownership
        .selection_sources
        .span_or_empty(receipt.sources)
        .iter()
        .any(|source| source.symbol == loan.owner_symbol)
        || !retirement
            .order
            .precedes(loan.statement_index, receipt.statement_ordinal as usize)
    {
        return false;
    }
    let Some(reference) = validation::reference_result_custody::storage_owner_type_reference(
        retirement.program,
        retirement.state,
        loan.owner_symbol,
        receipt.statement_ordinal,
    ) else {
        return false;
    };
    let path = retirement.facts.borrow.loan_owner_path(loan);
    if super::transport::carrier_path(retirement.program, reference, path)
        .is_none_or(|selected| selected != complement.path)
    {
        return false;
    }
    retirement
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, candidate)| {
            candidate.owner_symbol == loan.owner_symbol
                && retirement.order.precedes(
                    candidate.statement_index,
                    receipt.statement_ordinal as usize,
                )
                && retirement.facts.borrow.loan_owner_path(candidate) == path
        })
        .map(|(_, candidate)| candidate.statement_index)
        .max_by_key(|statement| retirement.order.position(*statement))
        == Some(loan.statement_index)
}

pub(super) fn supplies_group(
    retirement: &Retirement<'_>,
    supplier: Handle<BorrowLoanFact>,
    receivers: HandleSpan<BorrowLoanFact>,
) -> bool {
    let Some(group) = retirement.facts.borrow.loan_group(receivers) else {
        return false;
    };
    let mut pending = group.map(|(handle, _)| handle).collect::<Vec<_>>();
    let mut visited = Vec::new();
    while let Some(receiver) = pending.pop() {
        if receiver == supplier {
            return true;
        }
        if visited.contains(&receiver) {
            continue;
        }
        visited.push(receiver);
        pending.extend(
            retirement
                .transported_loans
                .iter()
                .filter_map(|(received, supplying)| (*received == receiver).then_some(*supplying)),
        );
    }
    false
}

pub(in crate::execution::terminal_unit::control::statement_sequence) fn closed_complements(
    retirement: &Retirement<'_>,
) -> Option<Vec<CheckedUnitSelectedReferenceRetirementPlan>> {
    let mut output = Vec::new();
    for (selection, receipt) in retirement.facts.flow.ownership.owned_selections.iter() {
        if receipt.machine != retirement.machine
            || receipt.state != retirement.state.symbol
            || receipt.statement_ordinal >= retirement.statement_index
            || !receipt.destination.is_valid()
        {
            continue;
        }
        let Some(groups) = validation::reference_result_custody::local_record_loans(
            retirement.program,
            retirement.facts,
            retirement.machine,
            retirement.state,
            receipt.statement_ordinal,
        ) else {
            continue;
        };
        for (source, loans) in groups {
            if retirement
                .operations
                .iter()
                .chain(retirement.pending)
                .any(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference {
                        selection_complements, ..
                    } if selection_complements.iter().any(|complement| {
                        complement.selection == selection && complement.path == source.path
                    }))
                })
            {
                continue;
            }
            if !retirement.facts.borrow.loan_group(loans)?.any(|(loan, _)| {
                retirement.pending.iter().any(|operation| {
                    matches!(operation, CheckedUnitEffectOperationPlan::ReleaseReference { loans: released, .. }
                        if supplies_group(retirement, loan, *released))
                })
            }) {
                continue;
            }
            if retirement.facts.borrow.loan_group(loans)?.all(|(loan, _)| {
                retirement.holder_retired(loan) && retirement.children_retired(loan)
            }) {
                output.push(CheckedUnitSelectedReferenceRetirementPlan {
                    selection,
                    path: source.path,
                    loans,
                });
            }
        }
    }
    Some(output)
}
