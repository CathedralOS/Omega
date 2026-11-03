//! A normal provider completion supplies an opaque descriptor permission.
//! It has no caller-owned root activation or reborrow restoration interval.
//! Validate its published completion against the exact initialized slot and
//! supplying invocation. Syntactic call rows alone need not have executed;
//! use sites still require their own active cut and retained supplier lineage.

use checked_trees::{BorrowFacts, BorrowLoanLineage, FlowFacts};
use diagnostics::Diagnostic;

mod completions;
mod inputs;

pub(super) use completions::install;

/// The namespace is established only by this exact call's normal exit.
/// Later uses separately require a current holder or a captured descendant.
pub(super) fn normal_call(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<arena::Handle<checked_trees::FlowCallFact>> {
    let source = borrow.supplied_output_call(loan)?;
    let invocation = borrow.calls.get(source);
    let incoming = borrow
        .incoming_references
        .get(borrow.loans.get(loan).incoming_reference);
    let state = flow
        .control
        .unique_state(incoming.machine_symbol, incoming.state_symbol)?;
    let mut calls = state.calls.handles().filter(|handle| {
        let actual = flow.control.calls.get(*handle);
        actual.statement_index == invocation.statement_index
            && actual.call_ordinal == invocation.call_ordinal
            && actual.target_symbol == invocation.target_symbol
            && actual.authored_expression == invocation.authored_expression
            && flow
                .borrow_loan_constraints(actual.exit_constraints)
                .any(|live| live == loan)
    });
    let actual = calls.next()?;
    calls.next().is_none().then_some(actual)
}

pub(super) fn validate_normal_output_completions(
    borrow: &BorrowFacts,
    flow: &FlowFacts,
) -> Result<(), Vec<Diagnostic>> {
    if !borrow
        .loans
        .iter()
        .any(|(_, loan)| loan.lineage == BorrowLoanLineage::SuppliedOutput)
    {
        return Ok(());
    }
    let mut diagnostics = Vec::new();
    for (_, state) in flow.control.states.iter() {
        for actual in flow.control.calls.span_or_empty(state.calls) {
            let Some(source) = borrow.call_at(
                state.machine_symbol,
                state.state_symbol,
                actual.statement_index,
                actual.call_ordinal,
            ) else {
                continue;
            };
            let invocation = borrow.calls.get(source);
            // Only a normal flow cut publishes supplied permissions. A
            // dormant syntactic call cannot demand a local activation, and
            // its declaration is not evidence of a performed completion.
            for handle in invocation.completed_reference_loans.handles() {
                if !borrow.loans.is_valid(handle) {
                    continue;
                }
                let loan = borrow.loans.get(handle);
                if loan.lineage != BorrowLoanLineage::SuppliedOutput
                    || !flow
                        .borrow_loan_constraints(actual.exit_constraints)
                        .any(|live| live == handle)
                {
                    continue;
                }
                if actual.target_symbol != invocation.target_symbol
                    || actual.authored_expression != invocation.authored_expression
                    || loan.established_by_call != source
                    || borrow.supplied_output_call(handle) != Some(source)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "supplied output permission has no exact normal-call and initialized-slot correspondence at state {:?}, statement {}, call {}",
                        state.state_symbol, actual.statement_index, actual.call_ordinal,
                    )));
                }
            }
        }
    }
    crate::checks::finish(diagnostics)
}
