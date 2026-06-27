mod arrays;
mod diagnostics;
mod expressions;
mod facts;
mod guards;
mod incoming_guards;
mod indexes;
mod initializers;
mod proofs;
mod requirements;
mod state_arguments;
mod statements;
mod types;

use arrays::fixed_array_field_lengths;
pub(in crate::checks) use arrays::fixed_array_type_length;
use facts::RangeFacts;
use incoming_guards::{collect_incoming_guard_facts, seed_incoming_guard_facts};
use initializers::seed_field_integer_facts;
use omega_core::diagnostics::Diagnostic;
use requirements::seed_machine_requires;
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};
use statements::check_statement;

pub(crate) fn check_indexed_accesses(
    program: &omega_typed_trees::TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        let state_argument_facts = collect_state_argument_facts(program, &field_lengths, machine);
        let incoming_guard_facts = collect_incoming_guard_facts(program, machine);
        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(&field_lengths);
            seed_field_integer_facts(program, &mut facts, machine);
            seed_machine_requires(program, &mut facts, machine);
            seed_state_argument_facts(&mut facts, state, &state_argument_facts);
            seed_incoming_guard_facts(program, &mut facts, state, &incoming_guard_facts);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(
                    program,
                    machine,
                    state,
                    &mut facts,
                    statement,
                    &mut diagnostics,
                );
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
