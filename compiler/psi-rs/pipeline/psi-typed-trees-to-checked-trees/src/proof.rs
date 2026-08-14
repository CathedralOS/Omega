use crate::context::*;
use psi_checked_trees::CheckedEvidenceTerm;
mod contracts;
mod obligations;

use contracts::{
    append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_state_contract_facts, append_state_signature_contract_facts, build_contract_call_facts,
    build_contract_exit_facts, build_contract_operator_use_facts, estimated_contract_fact_capacity,
};
use obligations::lower_proof_obligation;

#[cfg(test)]
pub(crate) fn build_proof_facts(
    program: &psi_typed_trees::TypedTrees,
    proof_plan: &psi_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
) -> ProofFacts {
    build_proof_facts_with_operators(
        program,
        proof_plan,
        borrow,
        &CheckedOperatorFacts::default(),
    )
}

pub(crate) fn build_proof_facts_with_operators(
    program: &psi_typed_trees::TypedTrees,
    proof_plan: &psi_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
    operators: &CheckedOperatorFacts,
) -> ProofFacts {
    let mut obligations = psi_arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts =
        psi_arena::Arena::with_capacity(estimated_contract_fact_capacity(program));
    let mut evidence_terms = psi_arena::Arena::default();

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(lower_proof_obligation(obligation));
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts, &mut evidence_terms);
        for state in program.machine_states(machine) {
            append_state_contract_facts(program, machine, state, &mut contract_facts);
        }
        append_inherited_trait_contract_facts(program, machine, &mut contract_facts);
        for parameter in program.machine_type_parameters(machine) {
            let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
            else {
                continue;
            };
            append_state_signature_contract_facts(
                program,
                parameter.symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
            );
        }
    }
    for trait_definition in program.traits() {
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
        );
    }
    let (mut contract_fact_refs, contract_calls) =
        build_contract_call_facts(program, borrow, &contract_facts);
    let contract_operator_uses = build_contract_operator_use_facts(
        program,
        operators,
        &mut contract_facts,
        &mut contract_fact_refs,
    );
    let contract_exits =
        build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);
    let proposition_vocabulary = build_checked_proposition_vocabulary(program);

    ProofFacts::with_roots(
        obligations,
        contract_facts,
        evidence_terms,
        psi_arena::Arena::default(),
        psi_arena::Arena::default(),
        contract_fact_refs,
        contract_calls,
        contract_exits,
        contract_operator_uses,
        proposition_vocabulary,
    )
}

pub(crate) fn bind_evidence_forwarding_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &mut ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut forwardings = psi_arena::Arena::default();

    for forwarding in &program.evidence_forwardings {
        let output = evidence_term_named(
            &proof.evidence_terms,
            forwarding.machine_symbol,
            forwarding.target.as_str(),
            ContractProofFactKind::Ensures,
        );
        let source = evidence_term_named(
            &proof.evidence_terms,
            forwarding.machine_symbol,
            forwarding.source.as_str(),
            ContractProofFactKind::Requires,
        );
        let Some(output) = output else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "evidence forwarding target `{}` is not a named ensures binding of this machine",
                forwarding.target
            )));
            continue;
        };
        let Some(source) = source else {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "evidence forwarding source `{}` is not a named requires binding of this machine",
                forwarding.source
            )));
            continue;
        };
        let output_term = proof.evidence_terms.get(output);
        let source_term = proof.evidence_terms.get(source);
        if output_term.proposition != source_term.proposition
            || output_term.evidence_type != source_term.evidence_type
        {
            diagnostics.push(psi_diagnostics::Diagnostic::error(format!(
                "cannot forward evidence term `{}` into `{}` because their proposition identities differ",
                forwarding.source, forwarding.target
            )));
            continue;
        }
        forwardings.append(psi_checked_trees::EvidenceForwardingFact {
            machine_symbol: forwarding.machine_symbol,
            state_symbol: forwarding.state_symbol,
            output,
            source,
        });
    }

    if diagnostics.is_empty() {
        proof.evidence_forwardings = forwardings;
        validate_evidence_forwarding_definite_assignment(program, proof)
    } else {
        Err(diagnostics)
    }
}

fn validate_evidence_forwarding_definite_assignment(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    use psi_typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
    use std::collections::{BTreeSet, VecDeque};

    let mut diagnostic_messages = BTreeSet::new();

    for machine in program.machines() {
        let outputs = proof
            .evidence_terms
            .iter()
            .filter_map(|(handle, term)| {
                (term.owner
                    == (ContractProofFactOwner::Machine {
                        machine_symbol: machine.symbol,
                    })
                    && term.kind == ContractProofFactKind::Ensures)
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        if outputs.is_empty() {
            continue;
        }
        let states = program.machine_states(machine);
        let Some(entry) = states.first() else {
            continue;
        };
        let mut work = VecDeque::from([(entry.symbol, BTreeSet::<u32>::new())]);
        let mut seen = BTreeSet::new();

        while let Some((state_symbol, mut assigned)) = work.pop_front() {
            let key = (
                state_symbol.arena_index(),
                assigned.iter().copied().collect::<Vec<_>>(),
            );
            if !seen.insert(key) {
                continue;
            }
            let Some(state) = states.iter().find(|state| state.symbol == state_symbol) else {
                continue;
            };

            let assignments = proof
                .evidence_forwardings
                .iter()
                .filter_map(|(_, forwarding)| {
                    (forwarding.machine_symbol == machine.symbol
                        && forwarding.state_symbol == state.symbol)
                        .then_some(forwarding)
                })
                .collect::<Vec<_>>();
            for forwarding in assignments {
                if !assigned.insert(forwarding.output.arena_index()) {
                    let term = proof.evidence_terms.get(forwarding.output);
                    diagnostic_messages.insert(format!(
                        "named ensures evidence `{}` is assigned more than once on a reachable path through {}::{}",
                        term.name, machine.name, state.name
                    ));
                }
            }

            let mut has_transition = false;
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                has_transition = true;
                if transition.exit != TransitionExit::Ordinary {
                    continue;
                }
                for target_handle in [transition.target, transition.continuation] {
                    if !target_handle.is_valid() {
                        continue;
                    }
                    match program.statement_table.transition_target(target_handle) {
                        TransitionTargetNode::Named { path, .. } => {
                            let target = if path.symbol == machine.symbol {
                                entry.symbol
                            } else {
                                path.symbol
                            };
                            if states.iter().any(|state| state.symbol == target) {
                                work.push_back((target, assigned.clone()));
                            } else {
                                append_missing_evidence_diagnostics(
                                    proof,
                                    machine,
                                    state,
                                    &outputs,
                                    &assigned,
                                    &mut diagnostic_messages,
                                );
                            }
                        }
                        TransitionTargetNode::SelfTarget => {
                            work.push_back((entry.symbol, assigned.clone()));
                        }
                        TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                            append_missing_evidence_diagnostics(
                                proof,
                                machine,
                                state,
                                &outputs,
                                &assigned,
                                &mut diagnostic_messages,
                            );
                        }
                    }
                }
            }

            if !has_transition {
                append_missing_evidence_diagnostics(
                    proof,
                    machine,
                    state,
                    &outputs,
                    &assigned,
                    &mut diagnostic_messages,
                );
            }
        }
    }

    if diagnostic_messages.is_empty() {
        Ok(())
    } else {
        Err(diagnostic_messages
            .into_iter()
            .map(psi_diagnostics::Diagnostic::error)
            .collect())
    }
}

fn append_missing_evidence_diagnostics(
    proof: &ProofFacts,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    outputs: &[psi_arena::Handle<CheckedEvidenceTerm>],
    assigned: &std::collections::BTreeSet<u32>,
    messages: &mut std::collections::BTreeSet<String>,
) {
    for output in outputs {
        if !assigned.contains(&output.arena_index()) {
            messages.insert(format!(
                "named ensures evidence `{}` is not definitely assigned on the ordinary exit through {}::{}",
                proof.evidence_terms.get(*output).name,
                machine.name,
                state.name
            ));
        }
    }
}

fn evidence_term_named(
    terms: &psi_arena::Arena<CheckedEvidenceTerm>,
    machine_symbol: SymbolHandle,
    name: &str,
    kind: ContractProofFactKind,
) -> Option<psi_arena::Handle<CheckedEvidenceTerm>> {
    terms.iter().find_map(|(handle, term)| {
        (term.owner == ContractProofFactOwner::Machine { machine_symbol }
            && term.kind == kind
            && term.name == name)
            .then_some(handle)
    })
}

fn build_checked_proposition_vocabulary(
    program: &psi_typed_trees::TypedTrees,
) -> psi_checked_trees::CheckedPropositionVocabulary {
    let declarations = program
        .propositions()
        .iter()
        .filter_map(|declaration| {
            let evidence = match declaration.body {
                psi_typed_trees::proposition::PropositionBody::Primitive => {
                    psi_checked_trees::CheckedPropositionEvidence::FactOnly
                }
                psi_typed_trees::proposition::PropositionBody::Witness { evidence } => {
                    psi_checked_trees::CheckedPropositionEvidence::Witness {
                        evidence_type: program.display_type_reference(evidence),
                    }
                }
                psi_typed_trees::proposition::PropositionBody::Transparent { .. } => return None,
            };
            let binders = program
                .proposition_binders(declaration)
                .iter()
                .map(|binder| psi_checked_trees::CheckedPropositionBinder {
                    name: binder.name.as_str().to_owned(),
                    kind: match binder.kind {
                        psi_typed_trees::proposition::PropositionBinderKind::Type => {
                            psi_checked_trees::CheckedPropositionBinderKind::Type
                        }
                        psi_typed_trees::proposition::PropositionBinderKind::Const {
                            type_reference,
                        } => psi_checked_trees::CheckedPropositionBinderKind::Const {
                            type_identity: program.display_type_reference(type_reference),
                        },
                        psi_typed_trees::proposition::PropositionBinderKind::Machine => {
                            psi_checked_trees::CheckedPropositionBinderKind::Machine
                        }
                    },
                })
                .collect();
            let parameter_types = program
                .proposition_parameters(declaration)
                .iter()
                .map(|parameter| program.display_type_reference(parameter.type_reference))
                .collect();
            Some(psi_checked_trees::CheckedPropositionDeclaration {
                symbol: declaration.symbol,
                name: declaration.name.as_str().to_owned(),
                binders,
                parameter_types,
                evidence,
            })
        })
        .collect();
    let applications = program
        .proof_facts
        .iter()
        .filter_map(|(_, fact)| {
            let psi_typed_trees::domain::ProofFact::Proposition(application) = fact else {
                return None;
            };
            let normalized = program.normalize_nominal_proposition_application(application)?;
            Some(lower_checked_proposition_application(normalized))
        })
        .collect();
    psi_checked_trees::CheckedPropositionVocabulary {
        declarations,
        applications,
    }
}

fn lower_checked_proposition_application(
    normalized: psi_typed_trees::proposition::NormalizedPropositionApplicationIdentity,
) -> psi_checked_trees::CheckedPropositionApplication {
    psi_checked_trees::CheckedPropositionApplication {
        declaration: normalized.declaration,
        binder_arguments: normalized
            .binder_arguments
            .into_iter()
            .map(
                |argument| psi_checked_trees::CheckedPropositionBinderArgument {
                    kind: match argument.kind {
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Type
                        }
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Const
                        }
                        psi_typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                            psi_checked_trees::CheckedPropositionBinderArgumentKind::Machine
                        }
                    },
                    identity: argument.identity,
                },
            )
            .collect(),
        arguments: normalized.arguments,
    }
}

fn fact_handles(
    facts: HandleSpan<psi_typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<psi_typed_trees::domain::ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}

fn contract_fact_kind(
    kind: &psi_typed_trees::signature::SignatureContractKind,
) -> Option<ContractProofFactKind> {
    match kind {
        psi_typed_trees::signature::SignatureContractKind::Requires => {
            Some(ContractProofFactKind::Requires)
        }
        psi_typed_trees::signature::SignatureContractKind::Ensures => {
            Some(ContractProofFactKind::Ensures)
        }
        psi_typed_trees::signature::SignatureContractKind::Boundary => {
            Some(ContractProofFactKind::Boundary)
        }
        psi_typed_trees::signature::SignatureContractKind::Crashes { .. } => None,
    }
}

pub(crate) use contracts::contract_target_from_state_symbol;
