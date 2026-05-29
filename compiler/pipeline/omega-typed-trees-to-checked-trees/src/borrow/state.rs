use crate::context::*;
use omega_checked_trees::BorrowLoanFact;

use super::calls::collect_statement_borrow_calls;
use super::last_uses::update_state_loan_last_uses;
use super::loans::statement_borrow_loan;
use super::roots::{append_state_writable_roots, mutable_parameter_count};
use super::tracker::StateLoanTracker;

pub(super) struct BorrowFactArenas<'arenas> {
    pub(super) writable_roots: &'arenas mut omega_core::arena::Arena<BorrowWritableRootFact>,
    pub(super) access_segments: &'arenas mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    pub(super) argument_accesses: &'arenas mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    pub(super) calls: &'arenas mut omega_core::arena::Arena<BorrowCallFact>,
    pub(super) loans: &'arenas mut omega_core::arena::Arena<BorrowLoanFact>,
    pub(super) states: &'arenas mut omega_core::arena::Arena<StateBorrowFact>,
}

pub(super) fn append_state_borrow_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    arenas: &mut BorrowFactArenas<'_>,
    state_loan_trackers: &mut Vec<StateLoanTracker>,
) {
    state_loan_trackers.clear();
    let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
    append_state_writable_roots(
        program,
        machine,
        state,
        arenas.writable_roots,
        &mut writable_roots_span,
    );

    let mut calls_span = omega_core::arena::HandleSpan::empty();
    let mut loans_span = omega_core::arena::HandleSpan::empty();
    for (statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        if let Some((owner_symbol, owner_name, place, source_owner_symbol, kind)) =
            statement_borrow_loan(
                program,
                state,
                statement_index,
                machine.symbol,
                statement,
                state_loan_trackers,
            )
        {
            let loan_segments = arenas.access_segments.insert_many(place.segments.clone());
            let handle = arenas.loans.append_to_span(
                &mut loans_span,
                BorrowLoanFact {
                    statement_index,
                    last_use_statement_index: statement_index,
                    owner_symbol,
                    source_owner_symbol,
                    root_symbol: place.root_symbol,
                    segments: loan_segments,
                    kind,
                },
            );
            state_loan_trackers.push(StateLoanTracker {
                handle,
                owner_symbol,
                owner_name,
                place,
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
        state.statement_nodes,
        arenas.calls.span_or_empty(calls_span),
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
