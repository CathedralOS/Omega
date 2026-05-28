use crate::context::*;
mod accesses;
mod calls;
mod last_uses;
mod loans;
mod roots;
mod tracker;

use crate::lookup::machine_state_count;
use calls::collect_statement_borrow_calls;
use last_uses::update_state_loan_last_uses;
use loans::statement_borrow_loan;
use roots::{append_state_writable_roots, estimated_borrow_root_capacity, mutable_parameter_count};
use tracker::StateLoanTracker;

pub(crate) fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut access_segments =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut loans =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));
    let mut state_loan_trackers = Vec::new();

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            state_loan_trackers.clear();
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            append_state_writable_roots(
                program,
                machine,
                state,
                &mut writable_roots,
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
                        &state_loan_trackers,
                    )
                {
                    let loan_segments = access_segments.insert_many(place.segments.clone());
                    let handle = loans.append_to_span(
                        &mut loans_span,
                        omega_checked_trees::BorrowLoanFact {
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
                    &mut access_segments,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            update_state_loan_last_uses(
                program,
                state.statement_nodes,
                calls.span_or_empty(calls_span),
                &argument_accesses,
                &state_loan_trackers,
                &mut loans,
            );

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count: mutable_parameter_count(program, state),
                calls: calls_span,
                loans: loans_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        access_segments,
        argument_accesses,
        calls,
        loans,
        states,
    }
}
