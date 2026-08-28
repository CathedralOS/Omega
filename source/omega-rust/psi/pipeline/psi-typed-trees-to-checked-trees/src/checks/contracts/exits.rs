use psi_checked_trees::{CheckFacts, FlowStateFact};
use psi_diagnostics::Diagnostic;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::{machine_name, semantic_fact_requirement_label};

pub(super) fn check_exit_ensures(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit_flow: &psi_checked_trees::FlowExitFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts: Vec<_> = facts
        .flow
        .contexts
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
            let proved = semantic_contexts_prove_contract_fact(
                program,
                &facts.semantic,
                &entry_contexts,
                fact,
            );
            let authorized_route = match fact.payload {
                psi_facts::FactPayload::ContractDomainMembership { domain_symbol, .. } => program
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == state_flow.machine_symbol)
                    .is_some_and(|machine| {
                        crate::qualification_evidence::machine_has_checked_domain_establishment(
                            program,
                            machine,
                            domain_symbol,
                        )
                    }),
                _ => false,
            };
            let route_predicates_satisfied = match fact.payload {
                psi_facts::FactPayload::ContractDomainMembership { domain_symbol, .. } => program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == domain_symbol)
                    .is_none_or(|domain| !domain.predicate_body.is_present() || proved),
                _ => true,
            };
            // Named witness-bearing `ensures` lanes are discharged by the
            // separate evidence-forwarding proof pass. That pass validates
            // exact term identity, proposition/interface agreement,
            // single-assignment, and definite assignment on every exit; the
            // ordinary semantic prover has no runtime witness fact to consume.
            let evidence_assignment = match fact.payload {
                psi_facts::FactPayload::ContractPropositionApplication {
                    fact: source_fact,
                    ..
                } => facts.proof.contract_facts.iter().any(|(_, contract)| {
                    contract.kind == psi_checked_trees::ContractProofFactKind::Ensures
                        && contract.owner
                            == (psi_checked_trees::ContractProofFactOwner::Machine {
                                machine_symbol: state_flow.machine_symbol,
                            })
                        && contract.fact == source_fact
                        && contract.evidence_term.is_some()
                }),
                _ => false,
            };
            let satisfied =
                proved || evidence_assignment || (authorized_route && route_predicates_satisfied);

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
