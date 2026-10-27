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
//! closure. Cited indexed induction judgments remain explicit portable proof
//! roots with their source contract and complete assumption closure. Imported
//! conclusions do not become execution facts: executable operations still need
//! their ordinary Terminal justification rows.

use checked_trees::CheckFacts;

/// Source roots remain available to checking; execution consumers exclude the
/// complete argument computation of these independently admitted citations.
pub(crate) struct ErasedCitationStatement {
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
}

pub(crate) fn is_fact_only_citation(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    statement: &typed_trees::statement::StatementNode,
) -> bool {
    validation::is_fact_only_citation(program, statement, &|machine| {
        facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    })
}

pub(crate) fn retire_fact_only_citations(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Vec<ErasedCitationStatement> {
    let mut erased = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                if is_fact_only_citation(program, facts, statement) {
                    erased.push(ErasedCitationStatement {
                        state: state.symbol,
                        statement_index,
                    });
                    if let Some(theorem) = statement.as_call().and_then(|call| {
                        validation::fact_only_theorem(program, call.target_symbol, &|machine| {
                            facts
                                .termination
                                .for_machine(machine)
                                .map(|plan| plan.checked_summary.clone())
                        })
                    }) {
                        if validation::mathematical_applications::is_indexed_logical_machine(
                            program, theorem,
                        ) {
                            let citation = checked_trees::CheckedIndexedCitation {
                                caller: machine.symbol,
                                theorem: theorem.symbol,
                            };
                            if !facts.proof.indexed_citations.contains(&citation) {
                                facts.proof.indexed_citations.push(citation);
                            }
                        }
                    }
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
    erased
}
