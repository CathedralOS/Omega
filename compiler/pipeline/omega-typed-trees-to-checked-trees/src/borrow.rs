use super::*;
mod accesses;
mod calls;
mod roots;

use calls::collect_statement_borrow_calls;
use crate::lookup::machine_state_count;
use roots::{append_state_writable_roots, estimated_borrow_root_capacity, mutable_parameter_count};

pub(crate) fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            append_state_writable_roots(
                program,
                machine,
                state,
                &mut writable_roots,
                &mut writable_roots_span,
            );

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count: mutable_parameter_count(program, state),
                calls: calls_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        argument_accesses,
        calls,
        states,
    }
}
