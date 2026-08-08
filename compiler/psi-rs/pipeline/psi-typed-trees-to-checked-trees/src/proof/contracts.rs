use super::*;
mod calls;
mod inherited;
mod operators;

pub(crate) use calls::{
    build_contract_call_facts, build_contract_exit_facts, contract_target_from_state_symbol,
};
pub(crate) use inherited::{
    append_inherited_trait_contract_facts, estimated_contract_fact_capacity,
};
pub(crate) use operators::build_contract_operator_use_facts;

pub(crate) fn append_machine_contract_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    contract_facts: &mut psi_arena::Arena<ContractProofFact>,
) {
    for contract in program.machine_contracts(machine) {
        let Some(kind) = super::contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in super::fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind,
                owner: ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                fact,
                qualification_authorization: None,
            });
        }
    }
}

pub(crate) fn append_state_contract_facts(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state: &psi_typed_trees::state::State,
    contract_facts: &mut psi_arena::Arena<ContractProofFact>,
) {
    for contract in program.state_contracts(state) {
        let Some(kind) = super::contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in super::fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind,
                owner: ContractProofFactOwner::MachineState {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                fact,
                qualification_authorization: None,
            });
        }
    }
}

pub(crate) fn append_state_signature_contract_facts(
    program: &psi_typed_trees::TypedTrees,
    owner_symbol: SymbolHandle,
    signatures: &[psi_typed_trees::signature::StateSignature],
    contract_facts: &mut psi_arena::Arena<ContractProofFact>,
) {
    for signature in signatures {
        for contract in program.state_signature_contracts(signature) {
            let Some(kind) = super::contract_fact_kind(&contract.kind) else {
                continue;
            };
            for fact in super::fact_handles(contract.facts) {
                let qualification_authorization =
                    crate::qualification_evidence::boundary_qualification_authorization(
                        program,
                        owner_symbol,
                        signature,
                        contract.kind.clone(),
                        fact,
                    );
                contract_facts.append(ContractProofFact {
                    kind,
                    owner: ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol: signature.symbol,
                    },
                    fact,
                    qualification_authorization,
                });
            }
        }
    }
}
