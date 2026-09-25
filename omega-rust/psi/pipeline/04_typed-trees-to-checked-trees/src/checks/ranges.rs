//! Index and range checks: every indexed access and subslice range in a
//! machine body must be proved within its collection's length from the facts
//! known at that statement.
//!
//! `check_indexed_accesses` is the entry;
//! `checks::check_checked_facts_recording_with_crash_admission` calls it after
//! the operator check and passes the incoming guard index it built from
//! `incoming_guards`. It reads the fixed-array field lengths once (`arrays`),
//! then for each machine prepares one `facts::RangeCallContext` per state and
//! collects the facts that edge arguments carry into each state
//! (`state_arguments`), the machine's incoming guards, and its loop
//! invariants (`loop_invariants`). For each state it starts a new
//! `facts::RangeFacts`, defines the state parameters as locals, and seeds, in
//! order: the state's `requires` (`requirements`), the state argument facts,
//! dependent parameter orderings (`dependent_params`), the incoming guard
//! facts and the loop invariant facts. It then runs
//! `statements::check_statement` on each statement.
//!
//! `check_statement` drives `statement_transfer::transfer_statement_facts`,
//! the single statement fact transfer that `state_arguments` also replays,
//! and checks each expression it reads with `indexes` and each transition
//! target under its edge's facts. `assignment_lengths` decides how an
//! assignment changes a collection's live length.
//!
//! The remaining children are shared vocabulary: `facts` is the per-state
//! fact store, `guards` seeds facts from guard expressions, `expressions`,
//! `types` and `proofs` answer integer, length, type and bound questions, and
//! `diagnostics` words the failures. `incoming_guards`, `requirements` and
//! `types` are also used by other checks, and `indexes` exports
//! `ranges_seam_owns` for the operator `requires` check.

// What a state starts with before its statements run: its `requires`, the
// facts its incoming edges' arguments carry, dependent parameter orderings,
// incoming guards and loop invariants.
mod dependent_params;
pub(in crate::checks) mod incoming_guards;
mod loop_invariants;
pub(in crate::checks) mod requirements;
mod state_arguments;

// Statement checking: the shared statement transfer, the statement check
// that drives it, and the index and assignment-length checks it runs.
mod assignment_lengths;
mod indexes;
pub(in crate::checks) use indexes::ranges_seam_owns;
mod statement_transfer;
mod statements;

// Shared vocabulary: the per-state fact store, guard seeding, integer,
// length, type and bound queries, and failure diagnostics.
mod arrays;
mod diagnostics;
mod expressions;
mod facts;
mod guards;
mod proofs;
pub(crate) mod types;

// Tests.
#[cfg(test)]
mod alias_lengths_tests;
#[cfg(test)]
mod cache_tests;

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
    operators: &checked_trees::CheckedOperatorFacts,
    borrows: &checked_trees::BorrowFacts,
    flow: &checked_trees::FlowFacts,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    incoming_guards: &IncomingGuardIndex,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    let field_lengths = fixed_array_field_lengths(program);
    let mut diagnostics = Vec::new();
    // All states and their branch snapshots query the same immutable program
    // and borrow facts through the pass's shared summary table.

    for machine in program.machines() {
        let calls: Vec<_> = program
            .machine_states(machine)
            .iter()
            .map(|state| facts::RangeCallContext::new(machine, state, borrows, flow, call_frames))
            .collect();
        let state_argument_facts = collect_state_argument_facts(
            program,
            &field_lengths,
            machine,
            call_frames,
            &calls,
            operators,
            mutation_summaries,
        );
        let incoming_guard_facts = incoming_guards.for_machine(machine.symbol);
        let loop_invariant_facts = collect_loop_invariant_facts(program, machine, call_frames);
        // One lazy whole-program bound index per machine: every state's
        // facts (and their branch clones) fill the same cell, so the bound
        // maps build at most once for the machine's walk. The cell lives
        // inside the machine loop because the per-state call contexts it
        // accompanies borrow the machine-local `calls` vector.
        let bound_lookup = std::rc::Rc::new(std::cell::RefCell::new(None));
        for (state, calls) in program.machine_states(machine).iter().zip(&calls) {
            let mut facts = RangeFacts::new(&field_lengths);
            facts.bound_program = Some(program);
            facts.bound_lookup = bound_lookup.clone();
            facts.mutation_summaries = std::borrow::Cow::Borrowed(mutation_summaries);
            facts.checked_operators = Some(operators);
            facts.checked_calls = Some(calls);
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
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                facts.statement_index = statement_index;
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
