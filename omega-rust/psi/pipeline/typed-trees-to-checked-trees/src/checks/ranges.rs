mod arrays;
mod dependent_params;
mod diagnostics;
mod expressions;
mod facts;
mod guards;
pub(in crate::checks) mod incoming_guards;
mod indexes;
mod loop_invariants;
mod proofs;
mod requirements;
mod state_arguments;
mod statements;
mod types;

use ::diagnostics::Diagnostic;
use arrays::fixed_array_field_lengths;
pub(in crate::checks) use arrays::fixed_array_type_length;
use dependent_params::seed_dependent_param_orderings;
use facts::RangeFacts;
use incoming_guards::{IncomingGuardIndex, seed_incoming_guard_facts};
use loop_invariants::{collect_loop_invariant_facts, seed_loop_invariant_facts};
use requirements::seed_state_requires;
use state_arguments::{collect_state_argument_facts, seed_state_argument_facts};
use statements::check_statement;
pub(in crate::checks) use types::expression_enforced_declared_range;

pub(crate) fn check_indexed_accesses(
    program: &typed_trees::TypedTrees,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();

    for machine in program.machines() {
        let state_argument_facts =
            collect_state_argument_facts(program, &field_lengths, machine, call_frames);
        let incoming_guard_facts = incoming_guards.for_machine(machine.symbol);
        let loop_invariant_facts = collect_loop_invariant_facts(program, machine, call_frames);
        for state in program.machine_states(machine) {
            let mut facts = RangeFacts::new(&field_lengths);
            // State parameters are stable named places for the duration of
            // the state, just like locals introduced by `let`. Retain a
            // literal fixed-array referee's length even through a reference
            // access mode so ordinary index checking—not an access-mode-
            // specific gate—owns every dynamic bounds obligation.
            for parameter in program.state_parameters(state) {
                facts.define_local(
                    parameter.symbol,
                    parameter.name.to_string(),
                    fixed_array_type_length(program, parameter.type_reference),
                    None,
                );
            }
            seed_state_requires(program, &mut facts, machine, state);
            seed_state_argument_facts(&mut facts, state, &state_argument_facts);
            seed_dependent_param_orderings(program, &mut facts, machine, state);
            seed_incoming_guard_facts(program, machine, &mut facts, state, incoming_guard_facts);
            seed_loop_invariant_facts(program, &mut facts, state, &loop_invariant_facts);
            for statement in program.statement_table.statements(state.statement_nodes) {
                check_statement(
                    program,
                    machine,
                    state,
                    call_frames,
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
