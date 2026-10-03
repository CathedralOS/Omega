//! Bridge the arithmetic entailment engine's postcondition stand-downs to the
//! flow checker's existing live-context judgment: a membership's exact subject
//! and domain, or a scalar predicate proved by the entry contexts or the exit's
//! exact return and live place values, or the shared all-arrival component
//! induction judgment. Every normal exit is derived from the
//! checked flow, rather than supplied by the certificate. An authored
//! postcondition, an entry annotation, or a partial exit roster is not proof,
//! and a predicate its own `requires` discharges keeps that certificate.
//!
//! This query consumes compiler-issued checked facts. It is local source
//! checking, not a portable proof kernel: package capture must bind the inputs
//! to immutable checked compilation custody before projecting a discharge.

use checked_trees::{
    CheckFacts, CheckedContractEntailmentExitMembershipDischarge, CheckedContractMembershipExit,
};
use typed_trees::{TypedTrees, domain::ProofFact, signature::SignatureContractKind};
use validation::{ContractEntailmentStandDown, ContractEntailmentStandDownReason};

/// Reconstruct postconditions supported by live facts at every exit.
/// Unsupported guarantees keep their original open obligation.
pub fn derive_contract_entailment_exit_membership_discharges(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<CheckedContractEntailmentExitMembershipDischarge> {
    let call_frames = validation::CallFrameResolver::new(program);
    let mut cyclic_headers = super::exits::CyclicHeaderInvariants::new();
    validation::collect_contract_entailment_stand_downs(program)
        .into_iter()
        .filter_map(|stand_down| {
            reconstruct(
                program,
                facts,
                call_frames.as_ref(),
                &mut cyclic_headers,
                stand_down,
            )
        })
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

/// A predicate spelled verbatim in the machine's own `requires` closes by
/// its assumption discharge certificate; exit evidence never doubles it.
fn assumption_discharged(facts: &CheckFacts, stand_down: &ContractEntailmentStandDown) -> bool {
    facts
        .proof
        .contract_entailment_assumption_discharges
        .iter()
        .any(|certificate| {
            certificate.machine_symbol() == stand_down.machine_symbol
                && usize::try_from(certificate.contract_position())
                    .is_ok_and(|position| position == stand_down.contract_index)
                && usize::try_from(certificate.fact_position())
                    .is_ok_and(|position| position == stand_down.fact_index)
        })
}

/// What one stood-down postcondition requires of each exit's live facts.
#[derive(Clone, Copy)]
enum ExitGoal {
    /// The exact subject in this domain.
    Membership(symbols::SymbolHandle),
    /// The exact authored Boolean predicate.
    Predicate,
}

fn reconstruct<'program>(
    program: &'program TypedTrees,
    facts: &CheckFacts,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    cyclic_headers: &mut super::exits::CyclicHeaderInvariants,
    stand_down: ContractEntailmentStandDown,
) -> Option<CheckedContractEntailmentExitMembershipDischarge> {
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
    // A membership stands down as an unsupported fact; a scalar predicate
    // over post-state storage stands down outside the entailment language.
    let goal = match program
        .proof_facts
        .span_or_empty(contract.facts)
        .get(stand_down.fact_index)?
    {
        ProofFact::Membership(membership)
            if stand_down.reason == ContractEntailmentStandDownReason::UnsupportedEnsuresFact =>
        {
            ExitGoal::Membership(membership.domain_symbol)
        }
        ProofFact::Expression(_)
            if matches!(
                stand_down.reason,
                ContractEntailmentStandDownReason::OutsideEntailmentLanguage
                    | ContractEntailmentStandDownReason::UnrecognizedInductiveBody
            ) && !assumption_discharged(facts, &stand_down) =>
        {
            ExitGoal::Predicate
        }
        _ => return None,
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
            if exit.machine_symbol != machine.symbol || exit.state_symbol != state.state_symbol {
                return None;
            }
            let exact_origins = !facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(exit.parameter_origins)
                .iter()
                .any(|origin| origin.contract == source_fact && !origin.state_parameter.is_valid());
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
                    let exact = match (fact.payload, goal) {
                        (
                            facts::FactPayload::ContractDomainMembership {
                                kind: facts::ContractFactKind::Ensures,
                                fact: source,
                                domain_symbol,
                                ..
                            },
                            ExitGoal::Membership(goal_domain),
                        ) => source == source_fact && domain_symbol == goal_domain,
                        (
                            facts::FactPayload::ContractBooleanExpression {
                                kind: facts::ContractFactKind::Ensures,
                                fact: source,
                                ..
                            },
                            ExitGoal::Predicate,
                        ) => source == source_fact,
                        _ => false,
                    };
                    if exact {
                        matched = true;
                        let proved = match goal {
                            ExitGoal::Membership(_) => {
                                exact_origins
                                    && super::prover::semantic_contexts_prove_contract_fact(
                                        program,
                                        &facts.semantic,
                                        &entry_contexts,
                                        fact,
                                    )
                            }
                            ExitGoal::Predicate => {
                                // Induction proves the invocation-relative
                                // predicate without pretending the drifting
                                // state parameters have exact incoming origins.
                                // It rechecks the same complete component used
                                // by ordinary exit checking, never the mere
                                // presence of an authored guarantee or rank.
                                (exact_origins
                                    && super::exits::live_contexts_prove_scalar_postcondition(
                                        program,
                                        facts,
                                        exit,
                                        &entry_contexts,
                                        fact,
                                        call_frames,
                                    ))
                                    || cyclic_headers.proves(program, facts, exit, fact)
                            }
                        };
                        if !proved {
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
