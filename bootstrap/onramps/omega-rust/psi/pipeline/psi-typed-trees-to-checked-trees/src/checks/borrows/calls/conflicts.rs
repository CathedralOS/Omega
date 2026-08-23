use psi_checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, symbol_name};

use super::super::details::active_loan_detail;
use super::super::overlap::{
    borrow_access_overlaps_loan, borrow_accesses_overlap, canonical_place_overlaps_loan,
};

pub(super) fn check_call_access_conflicts(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: psi_arena::HandleSpan<psi_checked_trees::FlowConstraintRef>,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let accesses: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .collect();
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .map(|loan| (loan, facts.borrow.loans.get(loan)))
        .collect();

    for transferred in crate::flow::owned_call_operand_places(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call,
    ) {
        for (loan_handle, loan) in &active_loans {
            if !canonical_place_overlaps_loan(program, &transferred, loan, &facts.borrow) {
                continue;
            }
            let detail =
                active_loan_detail(state_flow, facts, *loan_handle, borrow_call.statement_index);
            diagnostics.push(Diagnostic::error(format!(
                "state `{target_name}` receives an owned value while local borrow `{}` is still active{}",
                symbol_name(program, loan.owner_symbol),
                detail
                    .map(|detail| format!(" ({detail})"))
                    .unwrap_or_default(),
            )));
        }
    }

    for (index, access) in accesses.iter().enumerate() {
        if !access.kind.is_exclusive() {
            continue;
        }

        for other_access in accesses.iter().skip(index + 1) {
            if !borrow_accesses_overlap(program, facts, access, other_access) {
                continue;
            }

            match other_access.kind {
                BorrowAccessKind::Mutable => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
                BorrowAccessKind::Read => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    borrow_access_label(program, &facts.borrow, access),
                ))),
                BorrowAccessKind::WriteOnly => diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives write-only `{}` overlapping another argument in the same call",
                    borrow_access_label(program, &facts.borrow, access)
                ))),
            }
        }

        for (loan_handle, loan) in &active_loans {
            if borrow_access_overlaps_loan(program, facts, access, loan) {
                let detail = active_loan_detail(
                    state_flow,
                    facts,
                    *loan_handle,
                    borrow_call.statement_index,
                );
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` while local borrow `{}` is still active{}",
                    borrow_access_label(program, &facts.borrow, access),
                    symbol_name(program, loan.owner_symbol),
                    detail
                        .map(|detail| format!(" ({detail})"))
                        .unwrap_or_default(),
                )));
            }
        }
    }
}
