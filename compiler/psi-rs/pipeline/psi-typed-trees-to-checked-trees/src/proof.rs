use crate::context::*;
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
        contract_fact_refs,
        contract_calls,
        contract_exits,
        contract_operator_uses,
        proposition_vocabulary,
    )
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
