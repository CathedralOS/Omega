//! Bridge the arithmetic entailment engine's membership stand-downs to the
//! flow checker's existing live-context judgment. Every normal exit is derived
//! from the checked flow, rather than supplied by the certificate. An authored
//! postcondition, an entry annotation, or a partial exit roster is not proof.
//!
//! This query consumes compiler-issued checked facts. It is local source
//! checking, not a portable proof kernel: package capture must bind the inputs
//! to immutable checked compilation custody before projecting a discharge.

use checked_trees::{
    CheckFacts, CheckedContractEntailmentExitMembershipDischarge, CheckedContractMembershipExit,
};
use typed_trees::{TypedTrees, domain::ProofFact, signature::SignatureContractKind};
use validation::{ContractEntailmentStandDown, ContractEntailmentStandDownReason};

/// Reconstruct membership guarantees supported by live facts at every exit.
/// Unsupported guarantees keep their original open obligation.
pub fn derive_contract_entailment_exit_membership_discharges(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<CheckedContractEntailmentExitMembershipDischarge> {
    validation::collect_contract_entailment_stand_downs(program)
        .into_iter()
        .filter_map(|stand_down| reconstruct(program, facts, stand_down))
        .collect()
}

/// Re-enumerate all exits against authoritative checked facts. The caller must
/// establish their custody; matching attacker-supplied facts is insufficient.
pub fn recheck_contract_entailment_exit_membership_discharge(
    program: &TypedTrees,
    facts: &CheckFacts,
    certificate: &CheckedContractEntailmentExitMembershipDischarge,
) -> Result<(), &'static str> {
    let reconstructed = derive_contract_entailment_exit_membership_discharges(program, facts)
        .into_iter()
        .find(|candidate| {
            candidate.machine_symbol() == certificate.machine_symbol()
                && candidate.contract_position() == certificate.contract_position()
                && candidate.fact_position() == certificate.fact_position()
        });
    if reconstructed.as_ref() == Some(certificate) {
        Ok(())
    } else {
        Err("exit membership discharge differs from complete live-exit reconstruction")
    }
}

fn reconstruct(
    program: &TypedTrees,
    facts: &CheckFacts,
    stand_down: ContractEntailmentStandDown,
) -> Option<CheckedContractEntailmentExitMembershipDischarge> {
    if stand_down.reason != ContractEntailmentStandDownReason::UnsupportedEnsuresFact {
        return None;
    }
    let machine = program.machines().iter().find(|machine| {
        machine.symbol == stand_down.machine_symbol
            && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
    })?;
    let contract = program
        .machine_contracts(machine)
        .get(stand_down.contract_index)?;
    if contract.kind != SignatureContractKind::Ensures {
        return None;
    }
    let ProofFact::Membership(membership) = program
        .proof_facts
        .span_or_empty(contract.facts)
        .get(stand_down.fact_index)?
    else {
        return None;
    };
    let source_fact = arena::Handle::from_parts(
        contract
            .facts
            .start()
            .arena_index()
            .checked_add(stand_down.fact_index.try_into().ok()?)?,
        contract.facts.start().generation(),
    );
    let commitment = facts.contract_plans.for_machine(machine.symbol)?.commitment;
    let mut covered_exits = Vec::new();
    for (_, state) in facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
    {
        for exit in facts.flow.control.exits.span_or_empty(state.exits) {
            if exit.machine_symbol != machine.symbol
                || exit.state_symbol != state.state_symbol
                || facts
                    .flow
                    .control
                    .exit_parameter_origins
                    .span_or_empty(exit.parameter_origins)
                    .iter()
                    .any(|origin| {
                        origin.contract == source_fact && !origin.state_parameter.is_valid()
                    })
            {
                return None;
            }
            let entry_contexts: Vec<_> = facts
                .flow
                .contexts
                .semantic_context_refs
                .span_or_empty(exit.entry_semantic_contexts)
                .iter()
                .map(|reference| reference.context)
                .collect();
            let mut matched = false;
            for context in facts
                .flow
                .semantic_constraint_contexts(exit.ensures_constraints)
            {
                for fact in facts
                    .semantic
                    .context_view(facts.semantic.contexts.get(context))
                    .facts()
                {
                    if matches!(fact.payload, facts::FactPayload::ContractDomainMembership {
                        kind: facts::ContractFactKind::Ensures, fact: source, domain_symbol, ..
                    } if source == source_fact && domain_symbol == membership.domain_symbol)
                    {
                        matched = true;
                        if !super::prover::semantic_contexts_prove_contract_fact(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            fact,
                        ) {
                            return None;
                        }
                    }
                }
            }
            if !matched {
                return None;
            }
            covered_exits.push(CheckedContractMembershipExit::new(
                state.state_symbol,
                exit.statement_index,
            ));
        }
    }
    CheckedContractEntailmentExitMembershipDischarge::new(
        machine.symbol,
        stand_down.contract_index.try_into().ok()?,
        stand_down.fact_index.try_into().ok()?,
        commitment,
        covered_exits,
    )
    .ok()
}
