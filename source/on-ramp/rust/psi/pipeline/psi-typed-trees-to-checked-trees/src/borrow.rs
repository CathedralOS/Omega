use crate::context::*;
pub(crate) mod accesses;
mod calls;
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
pub(crate) use tracker::BorrowOwnerSegment;

use crate::lookup::machine_state_count;
use roots::estimated_borrow_root_capacity;
use state::{BorrowFactArenas, append_state_borrow_facts};

pub(crate) fn build_borrow_facts(program: &psi_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        psi_arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        psi_arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut owner_segments =
        psi_arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        psi_arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls = psi_arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans = psi_arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = psi_arena::Arena::with_capacity(machine_state_count(program));
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
