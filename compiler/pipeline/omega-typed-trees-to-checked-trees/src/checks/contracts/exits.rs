use omega_checked_trees::{CheckFacts, FlowStateFact};
use omega_core::diagnostics::Diagnostic;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::{machine_name, semantic_fact_requirement_label};

pub(super) fn check_exit_ensures(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit_flow: &omega_checked_trees::FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts: Vec<_> = facts
        .flow
        .semantic_context_refs
        .span_or_empty(exit_flow.entry_semantic_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    for ensures_context in facts
        .flow
        .semantic_constraint_contexts(exit_flow.ensures_constraints)
    {
        let context = facts.semantic.contexts.get(ensures_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = semantic_contexts_prove_contract_fact(
                program,
                &facts.semantic,
                &entry_contexts,
                fact,
            );

            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove ensures contract for exit from {} at statement {}: {}",
                    machine_name(program, state_flow.machine_symbol),
                    exit_flow.statement_index,
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                )));
            }
        }
    }
}
