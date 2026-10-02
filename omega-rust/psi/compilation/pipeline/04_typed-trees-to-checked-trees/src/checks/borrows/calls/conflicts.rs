use checked_trees::{
    BorrowAccessKind, BorrowCallCompatibilityOperand, BorrowCallCompatibilitySubject,
    BorrowCallFact, CheckFacts, FlowStateFact,
};
use diagnostics::Diagnostic;

use crate::labels::{borrow_access_label, symbol_name};

use super::super::details::active_loan_detail;
use super::super::overlap::{StatedOrderingPremise, canonical_place_for_loan};
use super::evidence::{CallCompatibility, argument_operand, loan_operand};

pub(super) fn check_call_access_conflicts<'p>(
    program: &'p typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    target_name: &str,
    stated_premises: &[StatedOrderingPremise],
    diagnostics: &mut Vec<Diagnostic>,
    recording: &mut CallCompatibility<'_>,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) {
    let accesses: Vec<_> = facts
        .borrow
        .argument_accesses
        .span_or_empty(borrow_call.accesses)
        .iter()
        .collect();
    let data_access =
        super::receiver::explicit_jump_data_access(program, facts, state_flow, borrow_call);
    let exiting_source = super::super::statements::source_exiting_without_local_loan_carriers(
        program,
        state_flow,
        borrow_call,
    );
    let active_loans: Vec<_> = facts
        .flow
        .borrow_loan_constraints(entry_constraints)
        .filter(|loan| {
            // The same replayed scope closure applies to forwarded argument
            // authority and to the successor's mutations. Ordinary calls,
            // carried loans and unclosed observations remain active.
            !exiting_source.is_some_and(|source| {
                super::super::statements::local_loan_ends_before_successor(
                    program, facts, state_flow, source, *loan,
                )
            })
        })
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
                    bound_lookup,
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
            // A local jump forwards the activation's exact data parent. It
            // does not establish a second independent whole-record loan while
            // staging the other operands. The successor footprint judges each
            // retained child's lifetime; a completed copy holds no child loan.
            // Ordinary calls and pairs between other operands stay exclusive.
            if data_access == Some(index) || data_access == Some(other_index) {
                continue;
            }
            // A copied read or completed nested-call loan no longer observes
            // its source when the later access begins. A payload its arm's
            // dispatch copied before the call (end 0) ended before the earlier
            // access began. Retained loans and the active local loans below
            // still constrain both.
            if access
                .end_before_access
                .is_some_and(|end| end <= other_index)
                || other_access
                    .end_before_access
                    .is_some_and(|end| end <= index)
            {
                continue;
            }
            if recording.non_interfering(
                program,
                argument_operand(&facts.borrow, index, access),
                argument_operand(&facts.borrow, other_index, other_access),
                stated_premises,
                bound_lookup,
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
            // Owned carrier storage can transfer its loan to a successor while
            // retaining the backing home until the loan chain ends. A store
            // through borrowed storage has no such custody: the destination
            // survives independently of this activation and cannot retain its
            // local backing. Replay retains exact owner paths through aliases;
            // reference leaves are values, while crossing one leaves the
            // carrier's owned storage. Spatial compatibility alone cannot
            // establish this lifetime distinction.
            if super::super::statements::local_jump(program, state_flow, borrow_call).is_some()
                && access.end_before_access.is_none()
                && access.root_symbol == loan.owner_symbol
                && crate::borrow::owner_path_overlaps_place_segments(
                    program,
                    facts.borrow.loan_owner_path(loan),
                    facts.borrow.access_segments.span_or_empty(access.segments),
                )
                && program.symbols.get(loan.root_symbol).kind == symbols::SymbolKind::Local
                && program.symbols.get(loan.root_symbol).parent == state_flow.state_symbol
                && !crate::flow::loan_carrier_has_owned_storage(
                    program,
                    state_flow.state_symbol,
                    borrow_call.statement_index,
                    loan.owner_symbol,
                    facts.borrow.loan_owner_path(loan),
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "state `{target_name}` receives a carried loan of local `{}`, whose storage ends at the source state exit",
                    symbol_name(program, loan.root_symbol),
                )));
                continue;
            }
            if recording.non_interfering(
                program,
                argument_operand(&facts.borrow, index, access),
                loan_operand(&facts.borrow, *loan_handle, loan),
                stated_premises,
                bound_lookup,
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
