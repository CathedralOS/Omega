use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use super::prover::semantic_contexts_prove_contract_fact;
use crate::labels::{machine_name, semantic_fact_requirement_label};

mod scalars;

fn direct_result_float_meaning_reflexivity_proves_exit(
    facts: &CheckFacts,
    machine_symbol: symbols::SymbolHandle,
    fact: &facts::Fact,
) -> bool {
    let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
        return false;
    };
    facts
        .proof
        .direct_result_float_meaning_reflexivity(machine_symbol, expression)
        .is_some()
}

pub(super) fn check_exit_ensures(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    exit_flow: &checked_trees::FlowExitFact,
    entailment: &super::entailment::MachineEntailmentOutcome,
    content_plans: &[validation::ContentConservationSourcePlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
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
            let contract = match fact.payload {
                facts::FactPayload::ContractBooleanExpression { fact, .. }
                | facts::FactPayload::ContractDomainMembership { fact, .. }
                | facts::FactPayload::ContractCarryPermission { fact, .. }
                | facts::FactPayload::ContractPropositionApplication { fact, .. } => Some(fact),
                _ => None,
            };
            // An owner-authorized boundary result is established at the admitted
            // crossing. Its checked adapter does not originate that authority;
            // direct calls already refuse this authorization in call_contract_evidence.
            // Authored adapter guarantees have no such record and remain obligations.
            let admitted_boundary_result = facts.proof.contract_facts.iter().any(|(_, inherited)| {
                inherited.kind == checked_trees::ContractProofFactKind::Ensures
                    && Some(inherited.fact) == contract
                    && inherited.qualification_authorization.is_some()
                    && matches!(
                        inherited.owner,
                        checked_trees::ContractProofFactOwner::MachineState { machine_symbol, state_symbol }
                            if machine_symbol == state_flow.machine_symbol
                                && state_symbol == state_flow.state_symbol
                    )
            });
            if admitted_boundary_result {
                continue;
            }
            let missing_origins = facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(exit_flow.parameter_origins)
                .iter()
                .filter(|origin| {
                    Some(origin.contract) == contract && !origin.state_parameter.is_valid()
                })
                .map(|origin| program.symbols.name(origin.entry_parameter))
                .collect::<Vec<_>>();
            let proved = semantic_contexts_prove_contract_fact(
                program,
                &facts.semantic,
                &entry_contexts,
                fact,
            );
            let authorized_route = match fact.payload {
                facts::FactPayload::ContractDomainMembership { domain_symbol, .. } => program
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
                facts::FactPayload::ContractDomainMembership { domain_symbol, .. } => program
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
                facts::FactPayload::ContractPropositionApplication {
                    fact: source_fact, ..
                } => facts.proof.contract_facts.iter().any(|(_, contract)| {
                    contract.kind == checked_trees::ContractProofFactKind::Ensures
                        && contract.owner
                            == (checked_trees::ContractProofFactOwner::Machine {
                                machine_symbol: state_flow.machine_symbol,
                            })
                        && contract.fact == source_fact
                        && contract.evidence_term.is_some()
                }),
                _ => false,
            };
            let float_meaning_reflexivity = direct_result_float_meaning_reflexivity_proves_exit(
                facts,
                state_flow.machine_symbol,
                fact,
            );
            let checked_entailment = match fact.payload {
                facts::FactPayload::ContractBooleanExpression { expression, .. } => {
                    entailment.expressions.contains(&expression)
                }
                _ => false,
            };
            let satisfied = checked_entailment
                || evidence_assignment
                || (missing_origins.is_empty()
                    && (proved
                        || scalars::proves(
                            program,
                            facts,
                            exit_flow,
                            &entry_contexts,
                            fact,
                            call_frames,
                        )
                        || super::content_preservation::proves_exit(
                            program,
                            facts,
                            state_flow,
                            fact,
                            content_plans,
                        )
                        || super::integer_embeddings::proves_exit_equality(
                            program, state_flow, exit_flow, fact,
                        )
                        || float_meaning_reflexivity
                        || super::entailment::integral_parameter_reflexivity(program, fact)
                        || super::entailment::transparent_proposition_proves_exit(
                            program, entailment, state_flow, fact,
                        )
                        || (authorized_route && route_predicates_satisfied)));

            if !satisfied {
                let origin_diagnostic = if missing_origins.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; no exact incoming reference origin for {}",
                        missing_origins.join(", ")
                    )
                };
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove ensures contract for exit from {} at statement {}: {}{}",
                    machine_name(program, state_flow.machine_symbol),
                    exit_flow.statement_index,
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    origin_diagnostic,
                )));
            }
        }
    }
}
