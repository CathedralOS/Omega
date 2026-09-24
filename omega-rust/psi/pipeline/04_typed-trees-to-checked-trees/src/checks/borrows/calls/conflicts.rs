use checked_trees::{
    BorrowAccessKind, BorrowCallCompatibilityOperand, BorrowCallCompatibilitySubject,
    BorrowCallFact, CheckFacts, FlowStateFact,
};
use diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, symbol_name};

use super::super::details::active_loan_detail;
use super::super::overlap::{StatedOrderingPremise, canonical_place_for_loan};
use super::evidence::{CallCompatibility, argument_operand, loan_operand};

pub(super) fn check_call_access_conflicts(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    target_name: &str,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    recording: &mut CallCompatibility<'_>,
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

    for (transfer_ordinal, transferred) in crate::flow::owned_call_operand_places(
        program,
        &facts.operators,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call,
    )
    .into_iter()
    .enumerate()
    {
        for (loan_handle, loan) in &active_loans {
            if canonical_place_for_loan(&transferred, loan).is_some_and(|place| {
                recording.non_interfering(
                    program,
                    BorrowCallCompatibilityOperand {
                        subject: BorrowCallCompatibilitySubject::TransferredPlace(transfer_ordinal),
                        place,
                        access: BorrowAccessKind::Mutable,
                    },
                    loan_operand(&facts.borrow, *loan_handle, loan),
                    stated_premises,
                )
            }) {
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
        for (other_index, other_access) in accesses.iter().enumerate().skip(index + 1) {
            if recording.non_interfering(
                program,
                argument_operand(&facts.borrow, index, access),
                argument_operand(&facts.borrow, other_index, other_access),
                stated_premises,
            ) {
                continue;
            }

            match (&access.kind, &other_access.kind) {
                (BorrowAccessKind::Mutable, BorrowAccessKind::Mutable) => {
                    diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as mutable more than once",
                    borrow_access_label(program, &facts.borrow, access),
                )))
                }
                (BorrowAccessKind::Mutable, BorrowAccessKind::Read)
                | (BorrowAccessKind::Read, BorrowAccessKind::Mutable) => {
                    let mutable = if access.kind == BorrowAccessKind::Mutable {
                        access
                    } else {
                        other_access
                    };
                    diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives `{}` as both mutable and read-only",
                    borrow_access_label(program, &facts.borrow, mutable),
                )))
                }
                (BorrowAccessKind::WriteOnly, _)
                | (_, BorrowAccessKind::WriteOnly) => {
                    let write_only = if access.kind == BorrowAccessKind::WriteOnly {
                        access
                    } else {
                        other_access
                    };
                    diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives write-only `{}` overlapping another argument in the same call",
                    borrow_access_label(program, &facts.borrow, write_only)
                )))
                }
                (BorrowAccessKind::Read, BorrowAccessKind::Read) => {
                    diagnostics.push(Diagnostic::error(format!(
                        "state `{target_name}` receives dependent read-only places that do not establish non-interference"
                    )))
                }
            }
        }

        for (loan_handle, loan) in &active_loans {
            if recording.non_interfering(
                program,
                argument_operand(&facts.borrow, index, access),
                loan_operand(&facts.borrow, *loan_handle, loan),
                stated_premises,
            ) {
                continue;
            }
            let detail =
                active_loan_detail(state_flow, facts, *loan_handle, borrow_call.statement_index);
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
