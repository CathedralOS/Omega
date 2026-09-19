//! Checked proof facts: the evidence, proof-output and outcome facts bound
//! onto checked trees from proof plans.
//!
//! This file builds the proof facts. `evidence_forwarding.rs` binds
//! forwarded and projected evidence, `proof_output_calls.rs` binds
//! proof-output call facts and named witness lanes, `outcome_arms.rs` binds
//! outcome-specific arm facts, `proposition_vocabulary.rs` builds the
//! checked proposition vocabulary and lowers applications, `contracts.rs`,
//! `contract_entailment.rs`, `float_meaning.rs` and `obligations.rs` carry
//! contracts, entailment, float meaning and obligation lowering.

mod contract_entailment;
mod contracts;
mod evidence_forwarding;
mod float_meaning;
mod mathematical_declarations;
mod obligations;
mod outcome_arms;
mod proof_output_calls;
mod proposition_vocabulary;

pub(crate) use contract_entailment::build_contract_entailment_assumption_discharges;
pub use contract_entailment::{
    CheckedContractEntailmentAssumptionDischargeRecheckError,
    recheck_contract_entailment_assumption_discharge,
};
pub(crate) use contracts::contract_target_from_state_symbol;
pub(crate) use contracts::machine_parameter_evidence_signatures;
pub(crate) use evidence_forwarding::{
    bind_evidence_forwarding_facts, bind_evidence_projection_facts,
};
pub(crate) use float_meaning::bind_float_meaning_projection_facts;
pub(crate) use mathematical_declarations::build_checked_mathematical_declarations;
pub(crate) use outcome_arms::{bind_outcome_specific_arm_facts, exact_outcome_case_test};
pub(crate) use proof_output_calls::bind_proof_output_call_facts;
pub(crate) use proposition_vocabulary::lower_checked_proposition_application;

use checked_trees::CheckedEvidenceTerm;
use checked_trees::{
    BorrowFacts, CheckedOperatorFacts, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
};

use crate::proof::proposition_vocabulary::{
    build_checked_proposition_vocabulary, fact_handles, lower_checked_evidence_interface,
};
use contracts::{
    append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_operator_declaration_contract_facts, append_state_contract_facts,
    append_state_signature_contract_facts, build_contract_call_facts, build_contract_exit_facts,
    build_contract_operator_use_facts, estimated_contract_fact_capacity,
};
use obligations::lower_proof_obligation;

#[cfg(test)]
pub(crate) fn build_proof_facts(
    program: &typed_trees::TypedTrees,
    proof_plan: &proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
) -> ProofFacts {
    build_proof_facts_with_operators(
        program,
        proof_plan,
        borrow,
        &CheckedOperatorFacts::default(),
    )
    .expect("test fixture programs elaborate or are refused before fact construction")
}

pub(crate) fn build_proof_facts_with_operators(
    program: &typed_trees::TypedTrees,
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
        append_machine_contract_facts(program, machine, &mut contract_facts, &mut evidence_terms);
        let mut guarded_lane_position = 0usize;
        for contract in program.machine_contracts(machine) {
            let typed_trees::signature::SignatureContractKind::EnsuresForResultCase {
                result_data,
                result_case,
            } = &contract.kind
            else {
                continue;
            };
            for fact in fact_handles(contract.facts) {
                let evidence_term = contract.binding.as_ref().map(|binding| {
                    let typed_trees::domain::ProofFact::Proposition(application) =
                        program.proof_facts.get(fact)
                    else {
                        unreachable!("validated named guarded guarantee must bind a proposition")
                    };
                    let normalized = program
                        .normalize_nominal_proposition_application(application, None)
                        .expect("validated named guarded guarantee must have a nominal endpoint");
                    let (evidence_type, evidence_interface) = match &normalized.classification {
                        typed_trees::proposition::PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        } => (
                            evidence.clone(),
                            interface.as_ref().map(lower_checked_evidence_interface),
                        ),
                        typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                            unreachable!(
                                "validated named guarded guarantee must bind witness evidence"
                            )
                        }
                    };
                    let term = evidence_terms.append(CheckedEvidenceTerm {
                        name: binding.as_str().to_owned(),
                        owner: ContractProofFactOwner::Machine {
                            machine_symbol: machine.symbol,
                        },
                        kind: ContractProofFactKind::Ensures,
                        lane_position: guarded_lane_position,
                        proposition: lower_checked_proposition_application(normalized),
                        evidence_type,
                        evidence_interface,
                    });
                    guarded_lane_position += 1;
                    term
                });
                outcome_specific_guarantees.append(checked_trees::OutcomeSpecificGuaranteeFact {
                    machine_symbol: machine.symbol,
                    result_data: *result_data,
                    result_case: *result_case,
                    public_selector: contract
                        .binding
                        .as_ref()
                        .map(|binding| binding.as_str().to_owned()),
                    fact,
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
            );
        }
        append_inherited_trait_contract_facts(
            program,
            machine,
            &mut contract_facts,
            &mut inherited_contract_scopes,
            &evidence_terms,
        );
        for (owner_symbol, _, contract) in
            machine_parameter_evidence_signatures(program, program.machine_type_parameters(machine))
        {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for definition in program.data_definitions() {
        for (owner_symbol, _, contract) in
            machine_parameter_evidence_signatures(program, program.data_type_parameters(definition))
        {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for definition in program.domain_definitions() {
        for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
            program,
            program.domain_type_parameters(definition),
        ) {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
    }
    for trait_definition in program.traits() {
        for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
            program,
            program.trait_type_parameters(trait_definition),
        ) {
            append_state_signature_contract_facts(
                program,
                owner_symbol,
                std::slice::from_ref(contract),
                &mut contract_facts,
                &mut evidence_terms,
            );
        }
        for requirement in program.trait_machine_signatures(trait_definition) {
            for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(
                program,
                program.state_signature_type_parameters(requirement),
            ) {
                append_state_signature_contract_facts(
                    program,
                    owner_symbol,
                    std::slice::from_ref(contract),
                    &mut contract_facts,
                    &mut evidence_terms,
                );
            }
        }
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
            &mut evidence_terms,
        );
    }
    for operator in program.operators().iter().chain(
        program
            .domain_definitions()
            .iter()
            .flat_map(|domain| program.domain_operators(domain)),
    ) {
        append_operator_declaration_contract_facts(program, operator, &mut contract_facts);
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
    let mathematical_declarations = build_checked_mathematical_declarations(program)?;

    Ok(ProofFacts {
        obligations,
        contract_facts,
        inherited_contract_scopes,
        outcome_specific_guarantees,
        evidence_terms,
        contract_fact_refs,
        contract_calls,
        contract_exits,
        contract_operator_uses,
        proposition_vocabulary,
        mathematical_declarations,
        ..ProofFacts::default()
    })
}
