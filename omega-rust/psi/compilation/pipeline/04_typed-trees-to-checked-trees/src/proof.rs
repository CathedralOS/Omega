//! Checked proof facts: the evidence, proof-output and outcome facts bound
//! onto checked trees from proof plans.
//!
//! This file builds the proof facts. `evidence_forwarding.rs` binds
//! forwarded and projected evidence, `proof_output_calls.rs` binds
//! proof-output call facts and named witness lanes, `outcome_arms.rs` binds
//! outcome-specific arm facts, `proposition_vocabulary.rs` builds the
//! checked proposition vocabulary and lowers applications, `contracts.rs`,
//! `float_meaning.rs` and `obligations.rs` carry contracts, float meaning and
//! obligation lowering.
//! Mathematical declaration elaboration belongs to the independent proof
//! package; this owner only transfers its completed records into `ProofFacts`.

pub(crate) mod contracts;
mod evidence_forwarding;
mod float_meaning;
mod obligations;
mod outcome_arms;
mod proof_output_calls;
mod proposition_vocabulary;

pub(crate) use contracts::contract_target_from_state_symbol;
pub(crate) use contracts::machine_parameter_evidence_signatures;
pub(crate) use evidence_forwarding::{
    bind_evidence_forwarding_facts, bind_evidence_projection_facts,
};
pub(crate) use float_meaning::bind_float_meaning_projection_facts;
pub(crate) use outcome_arms::{bind_outcome_specific_arm_facts, exact_outcome_case_test};
pub(crate) use proof_output_calls::bind_proof_output_call_facts;
pub(crate) use proposition_vocabulary::lower_checked_proposition_application;

use checked_trees::{
    BorrowFacts, CheckedOperatorFacts, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
};

use crate::proof::proposition_vocabulary::{build_checked_proposition_vocabulary, fact_handles};
use contracts::{
    ContractEvidenceLanes, append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_named_contract_evidence_term, append_operator_declaration_contract_facts,
    append_parameter_evidence_contract_facts, append_state_contract_facts,
    append_state_signature_contract_facts, build_contract_call_facts, build_contract_exit_facts,
    build_contract_operator_use_facts, estimated_contract_fact_capacity,
};
use obligations::lower_proof_obligation;

pub(crate) fn build_proof_facts(
    program: &dyn ::validation::CheckingInput,
    proof_plan: &proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
    operators: &CheckedOperatorFacts,
) -> Result<ProofFacts, Vec<diagnostics::Diagnostic>> {
    let mut obligations = arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts = arena::Arena::with_capacity(estimated_contract_fact_capacity(program));
    let mut inherited_contract_scopes = arena::Arena::default();
    let mut outcome_specific_guarantees = arena::Arena::default();
    let mut evidence_terms = arena::Arena::default();

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(lower_proof_obligation(obligation));
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts, &mut evidence_terms)
            .map_err(|error| vec![error])?;
        let mut guarded_lanes = ContractEvidenceLanes::default();
        for contract in program.machine_contracts(machine) {
            let typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = &contract.kind
            else {
                continue;
            };
            for fact in fact_handles(contract.facts) {
                let evidence_term = match contract.binding.as_ref() {
                    Some(binding) => Some(
                        append_named_contract_evidence_term(
                            program,
                            ContractProofFactOwner::Machine {
                                machine_symbol: machine.symbol,
                            },
                            ContractProofFactKind::Ensures,
                            guarded_lanes.next(ContractProofFactKind::Ensures),
                            fact,
                            binding.as_str(),
                            &mut evidence_terms,
                        )
                        .map_err(|error| vec![error])?,
                    ),
                    None => None,
                };
                outcome_specific_guarantees.append(checked_trees::OutcomeSpecificGuaranteeFact {
                    machine_symbol: machine.symbol,
                    result_data: *result_data,
                    result_case: *result_case,
                    public_selector: contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    fact,
                    fact_is_truth: matches!(
                        program.proof_facts.get(fact),
                        typed_trees::domain::ProofFact::Expression(expression)
                            if matches!(
                                program.expression_table.expression(*expression),
                                typed_trees::expression::ExpressionNode::Boolean(true)
                            )
                    ),
                    argument_roots: program
                        .proof_facts
                        .get(fact)
                        .as_proposition()
                        .and_then(|application| {
                            program
                                .machine_states(machine)
                                .first()
                                .map(|state| (application, state))
                        })
                        .map(|(application, state)| {
                            program
                                .expression_table
                                .expression_handles(application.arguments)
                                .iter()
                                .map(|argument| {
                                    outcome_arms::occurrence_root(program, state, *argument)
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    evidence_term,
                });
            }
        }
        for state in program.machine_states(machine) {
            append_state_contract_facts(
                program,
                machine,
                state,
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        append_inherited_trait_contract_facts(
            program,
            machine,
            &mut contract_facts,
            &mut inherited_contract_scopes,
            &evidence_terms,
        );
        append_parameter_evidence_contract_facts(
            program,
            program.machine_type_parameters(machine),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
    }
    for definition in program.data_definitions() {
        append_parameter_evidence_contract_facts(
            program,
            program.data_type_parameters(definition),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
    }
    for definition in program.domain_definitions() {
        append_parameter_evidence_contract_facts(
            program,
            program.domain_type_parameters(definition),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
    }
    for trait_definition in program.traits() {
        append_parameter_evidence_contract_facts(
            program,
            program.trait_type_parameters(trait_definition),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
        for requirement in program.trait_machine_signatures(trait_definition) {
            append_parameter_evidence_contract_facts(
                program,
                program.state_signature_type_parameters(requirement),
                &mut contract_facts,
                &mut evidence_terms,
            )
            .map_err(|error| vec![error])?;
        }
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
    }
    // Applied contracts need their own signature identity: using the authored
    // template as owner would combine facts from distinct const/type tuples.
    // Keep authored rows as well, since generic declarations are still checked.
    for application in program.closed_requirement_applications() {
        append_state_signature_contract_facts(
            program,
            application.declaring_trait,
            std::slice::from_ref(application.signature),
            &mut contract_facts,
            &mut evidence_terms,
        )
        .map_err(|error| vec![error])?;
    }
    for operator in program.all_operators() {
        append_operator_declaration_contract_facts(program, operator, &mut contract_facts);
    }
    let (mut contract_fact_refs, contract_calls, call_contract_signatures) =
        build_contract_call_facts(program, borrow, &mut contract_facts, &mut evidence_terms)?;
    let contract_operator_uses = build_contract_operator_use_facts(
        program,
        operators,
        &mut contract_facts,
        &mut contract_fact_refs,
    );
    let contract_exits =
        build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);
    let proposition_vocabulary = build_checked_proposition_vocabulary(program);

    Ok(ProofFacts {
        obligations,
        contract_facts,
        inherited_contract_scopes,
        outcome_specific_guarantees,
        evidence_terms,
        contract_fact_refs,
        contract_calls,
        call_contract_signatures,
        contract_exits,
        contract_operator_uses,
        proposition_vocabulary,
        ..ProofFacts::default()
    })
}
