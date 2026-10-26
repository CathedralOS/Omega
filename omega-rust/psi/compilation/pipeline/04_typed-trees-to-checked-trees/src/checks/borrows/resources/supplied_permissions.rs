//! A normal provider completion supplies an opaque descriptor permission.
//! It has no caller-owned root activation or reborrow restoration interval.
//! Validate its published completion against the exact initialized slot and
//! supplying invocation. Syntactic call rows alone need not have executed;
//! use sites still require their own active cut and retained supplier lineage.

use checked_trees::{BorrowFacts, BorrowLoanLineage, FlowFacts};
use diagnostics::Diagnostic;

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
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
