use crate::context::*;
use psi_checked_trees::BorrowLoanFact;

use super::calls::collect_statement_borrow_calls;
use super::last_uses::update_state_loan_last_uses;
use super::loans::statement_borrow_loans;
use super::roots::{append_state_writable_roots, mutable_parameter_count};
use super::tracker::StateLoanTracker;

pub(super) struct BorrowFactArenas<'arenas> {
    pub(super) writable_roots: &'arenas mut psi_arena::Arena<BorrowWritableRootFact>,
    pub(super) access_segments: &'arenas mut psi_arena::Arena<psi_facts::PlaceSegment>,
    pub(super) owner_segments:
        &'arenas mut psi_arena::Arena<psi_checked_trees::BorrowLoanOwnerSegment>,
    pub(super) argument_accesses: &'arenas mut psi_arena::Arena<BorrowArgumentAccessFact>,
    pub(super) calls: &'arenas mut psi_arena::Arena<BorrowCallFact>,
    pub(super) loans: &'arenas mut psi_arena::Arena<BorrowLoanFact>,
    pub(super) states: &'arenas mut psi_arena::Arena<StateBorrowFact>,
}

pub(super) fn append_state_borrow_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    arenas: &mut BorrowFactArenas<'_>,
    state_loan_trackers: &mut Vec<StateLoanTracker>,
) {
    state_loan_trackers.clear();
    let mut writable_roots_span = psi_arena::HandleSpan::empty();
    append_state_writable_roots(
        program,
        machine,
        state,
        arenas.writable_roots,
        &mut writable_roots_span,
    );

    let mut calls_span = psi_arena::HandleSpan::empty();
    let mut loans_span = psi_arena::HandleSpan::empty();
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        for pending in statement_borrow_loans(
            program,
            state,
            statement_index,
            machine.symbol,
            statement,
            state_loan_trackers,
        ) {
            let loan_segments = arenas
                .access_segments
                .insert_many(pending.place.segments.clone());
            let owner_path = arenas
                .owner_segments
                .insert_many(pending.owner_path.iter().copied());
            let handle = arenas.loans.append_to_span(
                &mut loans_span,
                BorrowLoanFact {
                    statement_index,
                    last_use_statement_index: statement_index,
                    owner_symbol: pending.owner_symbol,
                    owner_path,
                    source_owner_symbol: pending.source_owner_symbol,
                    root_symbol: pending.place.root_symbol,
                    segments: loan_segments,
                    kind: pending.kind.clone(),
                },
            );
            state_loan_trackers.push(StateLoanTracker {
                handle,
                owner_symbol: pending.owner_symbol,
                owner_name: pending.owner_name,
                kind: pending.kind,
                owner_path: pending.owner_path,
                place: pending.place,
            });
        }
        let mut call_ordinal = 0usize;
        collect_statement_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            statement,
            &mut call_ordinal,
            arenas.access_segments,
            arenas.argument_accesses,
            arenas.calls,
            &mut calls_span,
        );
    }

    update_state_loan_last_uses(
        program,
        state.symbol,
        state.statement_nodes,
        arenas.calls.span_or_empty(calls_span),
        arenas.access_segments,
        arenas.argument_accesses,
        state_loan_trackers,
        arenas.loans,
    );

    arenas.states.append(StateBorrowFact {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        writable_roots: writable_roots_span,
        mutable_parameter_count: mutable_parameter_count(program, state),
        calls: calls_span,
        loans: loans_span,
    });
}
