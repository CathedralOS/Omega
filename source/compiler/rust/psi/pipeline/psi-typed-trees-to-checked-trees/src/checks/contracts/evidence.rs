use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::{
    CheckFacts, CheckedEvidenceTerm, CheckedPropositionApplication, ContractEvidenceArgument,
    ContractProofFactKind, ContractProofFactOwner,
};
use psi_diagnostics::Diagnostic;

use crate::{call_site_evidence_arguments, call_target_parameters, find_call_site};

pub(super) fn bind_call_evidence_arguments(
    program: &psi_typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut bindings = psi_arena::Arena::default();

    let call_handles = facts
        .proof
        .contract_calls
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for call_handle in call_handles {
        let call = facts.proof.contract_calls.get(call_handle).clone();
        let Some(call_site) = find_call_site(
            program,
            call.caller_machine_symbol,
            call.caller_state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let authored = call_site_evidence_arguments(&call_site);
        let is_named_transition = matches!(call_site, crate::CallSite::TransitionNamed { .. });
        let mut parameters = facts
            .proof
            .contract_fact_refs
            .span_or_empty(call.requires)
            .iter()
            .filter_map(|fact_ref| {
                let fact = facts.proof.contract_facts.get(fact_ref.fact);
                if is_named_transition
                    && matches!(fact.owner, ContractProofFactOwner::Machine { .. })
                {
                    return None;
                }
                fact.evidence_term
            })
            .collect::<Vec<_>>();
        parameters
            .sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);

        if authored.len() != parameters.len() {
            diagnostics.push(Diagnostic::error(format!(
                "call `{}` supplies {} erased evidence argument{} but its named requires lane has {}",
                call_target_name(program, call.target_state_symbol),
                authored.len(),
                if authored.len() == 1 { "" } else { "s" },
                parameters.len(),
            )));
            continue;
        }

        let mut span = HandleSpan::empty();
        for (lane_position, (name, parameter)) in authored.iter().zip(parameters).enumerate() {
            let Some(source) = source_term_by_name(
                &facts.proof.evidence_terms,
                &facts.proof.outcome_specific_arms,
                call.caller_machine_symbol,
                call.caller_state_symbol,
                call.statement_index,
                name.as_str(),
            ) else {
                diagnostics.push(Diagnostic::error(format!(
                    "unknown incoming evidence term `{}` in call `{}`; erased arguments must name an explicit requires binding",
                    name,
                    call_target_name(program, call.target_state_symbol),
                )));
                continue;
            };

            let expected =
                instantiated_parameter_proposition(program, facts, &call, &call_site, parameter);
            let source_term = facts.proof.evidence_terms.get(source);
            if expected.as_ref() != Some(&source_term.proposition) {
                diagnostics.push(Diagnostic::error(format!(
                    "evidence term `{}` does not inhabit erased requires position {} of call `{}`",
                    name,
                    lane_position,
                    call_target_name(program, call.target_state_symbol),
                )));
                continue;
            }

            bindings.append_to_span(
                &mut span,
                ContractEvidenceArgument {
                    source,
                    parameter,
                    lane_position,
                },
            );
        }
        facts
            .proof
            .contract_calls
            .get_mut(call_handle)
            .evidence_arguments = span;
    }

    facts.proof.contract_evidence_arguments = bindings;
}

fn source_term_by_name(
    terms: &psi_arena::Arena<CheckedEvidenceTerm>,
    arms: &psi_arena::Arena<psi_checked_trees::OutcomeSpecificArmFact>,
    caller_machine_symbol: psi_symbols::SymbolHandle,
    caller_state_symbol: psi_symbols::SymbolHandle,
    statement_index: usize,
    name: &str,
) -> Option<Handle<CheckedEvidenceTerm>> {
    arms.iter()
        .filter(|(_, arm)| {
            arm.caller_machine_symbol == caller_machine_symbol
                && arm.caller_state_symbol == caller_state_symbol
                && arm.statement_index == statement_index
        })
        .flat_map(|(_, arm)| arm.rows.iter().filter_map(|row| row.selected_term))
        .find(|term| terms.get(*term).name == name)
        .or_else(|| {
            terms.iter().find_map(|(handle, term)| {
        let owner_matches = matches!(
            term.owner,
            ContractProofFactOwner::Machine { machine_symbol }
                if machine_symbol == caller_machine_symbol
        ) || matches!(
            term.owner,
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } if machine_symbol == caller_machine_symbol && state_symbol == caller_state_symbol
        );
        (owner_matches && term.kind == ContractProofFactKind::Requires && term.name == name)
            .then_some(handle)
    })
        })
}

fn instantiated_parameter_proposition(
    program: &psi_typed_trees::TypedTrees,
    facts: &CheckFacts,
    call: &psi_checked_trees::ContractCallFact,
    call_site: &crate::CallSite<'_>,
    parameter: Handle<CheckedEvidenceTerm>,
) -> Option<CheckedPropositionApplication> {
    let parameter_term = facts.proof.evidence_terms.get(parameter);
    let contract = facts
        .proof
        .contract_fact_refs
        .span_or_empty(call.requires)
        .iter()
        .map(|fact_ref| facts.proof.contract_facts.get(fact_ref.fact))
        .find(|contract| contract.evidence_term == Some(parameter))?;
    let psi_typed_trees::domain::ProofFact::Proposition(application) =
        program.proof_facts.get(contract.fact)
    else {
        return None;
    };
    let target_parameters = call_target_parameters(program, call.target_state_symbol)?;
    let argument_labels = program
        .expression_table
        .expression_handles(application.arguments)
        .iter()
        .map(|argument| {
            super::labels::instantiate_call_contract_expression_label(
                program,
                call.caller_state_symbol,
                call.statement_index,
                call_site,
                target_parameters,
                *argument,
            )
        })
        .collect();
    let mut proposition = parameter_term.proposition.clone();
    proposition.arguments = argument_labels;
    Some(proposition)
}

fn call_target_name(
    program: &psi_typed_trees::TypedTrees,
    target: psi_symbols::SymbolHandle,
) -> String {
    crate::labels::call_target_label(program, target)
}
