//! Borrow facts: the writable roots, loans, borrowing calls and argument
//! accesses of every state, recorded as `crate::checked_trees::BorrowFacts` for the
//! later fact builders and the borrow checks in `checks::borrows`.
//!
//! `build_borrow_facts` is the entry; `facts::build_check_facts` calls it
//! first. It sizes the arenas (`roots`), then for every state of every
//! machine `state::append_state_borrow_facts` records, in order: the state's
//! writable roots (`roots`); for each statement, the loans it forms
//! (`loans`), each tracked as a `tracker::StateLoanTracker`, and its
//! borrowing calls (`calls`, which records their argument accesses through
//! `accesses`); then each loan's last use (`last_uses`); and finally the
//! state's `StateBorrowFact` row.
//!
//! `view_link` resolves which input a returned view borrows; `loans` uses it
//! to link a call result's loan to its argument, and `checks::borrows` uses
//! it for the returned-view checks. `last_uses` and `loans` also export
//! queries used elsewhere in the crate.

use crate::checked_trees::BorrowFacts;
pub(crate) mod accesses;
pub(crate) mod calls;
mod last_uses;
mod loans;
mod roots;
mod state;
mod tracker;
pub(crate) mod view_link;

pub(crate) use last_uses::{
    place_is_used_after_statement, place_symbol_is_used_after_statement,
    place_symbol_is_used_in_state,
};
pub(crate) use loans::borrow_initializer_expressions;
pub(crate) use loans::call_declares_direct_view_source;
pub(crate) use loans::helper_call_borrow_loan_place;
pub(crate) use loans::types::reference_borrow_access_kind;
pub(crate) use tracker::BorrowOwnerSegment;

use crate::lookup::machine_state_count;
use roots::estimated_borrow_root_capacity;
use state::{BorrowFactArenas, append_state_borrow_facts};

pub(crate) fn build_borrow_facts(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
) -> BorrowFacts {
    let mut writable_roots = arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut owner_segments =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls = arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans = arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = arena::Arena::with_capacity(machine_state_count(program));
    let mut state_loan_trackers = Vec::new();

    {
        let mut arenas = BorrowFactArenas {
            writable_roots: &mut writable_roots,
            access_segments: &mut access_segments,
            owner_segments: &mut owner_segments,
            argument_accesses: &mut argument_accesses,
            calls: &mut calls,
            loans: &mut loans,
            states: &mut states,
        };
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                append_state_borrow_facts(
                    program,
                    machine,
                    state,
                    &mut arenas,
                    &mut state_loan_trackers,
                );
            }
        }
    }

    BorrowFacts::with_roots(
        writable_roots,
        access_segments,
        owner_segments,
        argument_accesses,
        calls,
        loans,
        states,
    )
}
