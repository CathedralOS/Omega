use crate::context::*;
mod contracts;
mod obligations;

use contracts::{
    append_inherited_trait_contract_facts, append_machine_contract_facts,
    append_state_signature_contract_facts, build_contract_call_facts, build_contract_exit_facts,
    estimated_contract_fact_capacity,
};
use obligations::lower_proof_obligation;

pub(crate) fn build_proof_facts(
    program: &omega_typed_trees::TypedTrees,
    proof_plan: &omega_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
) -> ProofFacts {
    let mut obligations = omega_core::arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts =
        omega_core::arena::Arena::with_capacity(estimated_contract_fact_capacity(program));

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(lower_proof_obligation(obligation));
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts);
        append_inherited_trait_contract_facts(program, machine, &mut contract_facts);
    }
    for trait_definition in program.traits() {
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
        );
    }
    for platform in program.platforms() {
        append_state_signature_contract_facts(
            program,
            platform.symbol,
            program.platform_state_signatures(platform),
            &mut contract_facts,
        );
    }
    let (mut contract_fact_refs, contract_calls) =
        build_contract_call_facts(program, borrow, &contract_facts);
    let contract_exits =
        build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);

    ProofFacts {
        obligations,
        contract_facts,
        contract_fact_refs,
        contract_calls,
        contract_exits,
    }
}

fn fact_handles(
    facts: HandleSpan<omega_typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<omega_typed_trees::domain::ProofFact>> {
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
    kind: omega_typed_trees::signature::SignatureContractKind,
) -> ContractProofFactKind {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => {
            ContractProofFactKind::Requires
        }
        omega_typed_trees::signature::SignatureContractKind::Ensures => {
            ContractProofFactKind::Ensures
        }
        omega_typed_trees::signature::SignatureContractKind::Boundary => {
            ContractProofFactKind::Boundary
        }
    }
}

pub(crate) use contracts::contract_target_from_state_symbol;
