//! Erase fact-only citations before execution planning.
//!
//! A statement call citing a theorem-only machine
//! (`validation::is_fact_only_citation`) emits no runtime call or work
//! ([citation](../../../../../../../wiki/spec/proofs/contracts.md#citation-and-induction)).
//! Checking has already discharged its `requires` and imported its `ensures`
//! from the call's flow row; that row stays in the arena because borrow
//! certificates and contexts hold its handles. Retiring it here is the one
//! erasure decision: planners skip retired rows, and statement sequencing
//! skips the citation statement itself, so the theorem never enters a unit
//! closure. Terminal verification cannot see the cited conclusions; the
//! executable operations must be justified by Terminal's own rows.

use checked_trees::CheckFacts;
use typed_trees::TypedTrees;

pub(crate) fn retire_fact_only_citations(program: &TypedTrees, facts: &mut CheckFacts) {
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                if validation::is_fact_only_citation(program, statement) {
                    facts.flow.control.retire_calls(
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        typed_trees::expression::ExpressionHandle::invalid(),
                        true,
                    );
                }
            }
        }
    }
}
