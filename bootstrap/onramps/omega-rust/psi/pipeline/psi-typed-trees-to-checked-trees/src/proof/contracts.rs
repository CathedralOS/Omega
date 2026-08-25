use super::*;
use psi_checked_trees::CheckedEvidenceTerm;
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
    evidence_terms: &mut psi_arena::Arena<CheckedEvidenceTerm>,
) {
    let mut requires_position = 0usize;
    let mut ensures_position = 0usize;
    for contract in program.machine_contracts(machine) {
        let Some(kind) = super::contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in super::fact_handles(contract.facts) {
            let evidence_term = contract.binding.as_ref().map(|binding| {
                let lane_position = match kind {
                    ContractProofFactKind::Requires => {
                        let position = requires_position;
                        requires_position += 1;
                        position
                    }
                    ContractProofFactKind::Ensures => {
                        let position = ensures_position;
                        ensures_position += 1;
                        position
                    }
                    ContractProofFactKind::Boundary => {
                        unreachable!("validated named contracts are requires or ensures")
                    }
                };
                let psi_typed_trees::domain::ProofFact::Proposition(application) =
                    program.proof_facts.get(fact)
                else {
                    unreachable!("validated named contract must bind a proposition")
                };
                let normalized = program
                    .normalize_nominal_proposition_application(application)
                    .expect("validated named contract must have a nominal proposition endpoint");
                let (evidence_type, evidence_interface) = match &normalized.classification {
                    psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => (
                        evidence.clone(),
                        interface
                            .as_ref()
                            .map(super::lower_checked_evidence_interface),
                    ),
                    psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                        unreachable!("validated named contract must bind witness evidence")
                    }
                };
                evidence_terms.append(CheckedEvidenceTerm {
                    name: binding.as_str().to_owned(),
                    owner: ContractProofFactOwner::Machine {
                        machine_symbol: machine.symbol,
                    },
                    kind,
                    lane_position,
                    proposition: super::lower_checked_proposition_application(normalized),
                    evidence_type,
                    evidence_interface,
                })
            });
            contract_facts.append(ContractProofFact {
                kind,
                owner: ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                fact,
                evidence_term,
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
    evidence_terms: &mut psi_arena::Arena<CheckedEvidenceTerm>,
) {
    let mut requires_position = 0usize;
    for contract in program.state_contracts(state) {
        let Some(kind) = super::contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in super::fact_handles(contract.facts) {
            let evidence_term =
                contract.binding.as_ref().map(|binding| {
                    let lane_position = match kind {
                        ContractProofFactKind::Requires => {
                            let position = requires_position;
                            requires_position += 1;
                            position
                        }
                        ContractProofFactKind::Ensures | ContractProofFactKind::Boundary => {
                            unreachable!("states admit only arrival requires contracts")
                        }
                    };
                    let psi_typed_trees::domain::ProofFact::Proposition(application) =
                        program.proof_facts.get(fact)
                    else {
                        unreachable!("validated named state contract must bind a proposition")
                    };
                    let normalized = program
                    .normalize_nominal_proposition_application(application)
                    .expect(
                        "validated named state contract must have a nominal proposition endpoint",
                    );
                    let (evidence_type, evidence_interface) = match &normalized.classification {
                    psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => (
                        evidence.clone(),
                        interface.as_ref().map(super::lower_checked_evidence_interface),
                    ),
                    psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                        unreachable!("validated named state contract must bind witness evidence")
                    }
                };
                    evidence_terms.append(CheckedEvidenceTerm {
                        name: binding.as_str().to_owned(),
                        owner: ContractProofFactOwner::MachineState {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                        },
                        kind,
                        lane_position,
                        proposition: super::lower_checked_proposition_application(normalized),
                        evidence_type,
                        evidence_interface,
                    })
                });
            contract_facts.append(ContractProofFact {
                kind,
                owner: ContractProofFactOwner::MachineState {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                },
                fact,
                evidence_term,
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
    evidence_terms: &mut psi_arena::Arena<CheckedEvidenceTerm>,
) {
    for signature in signatures {
        let owner = ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol: signature.symbol,
        };
        let mut requires_position = 0usize;
        let mut ensures_position = 0usize;
        for contract in program.state_signature_contracts(signature) {
            let Some(kind) = super::contract_fact_kind(&contract.kind) else {
                continue;
            };
            for fact in super::fact_handles(contract.facts) {
                let evidence_term = contract.binding.as_ref().map(|binding| {
                    let lane_position = match kind {
                        ContractProofFactKind::Requires => {
                            let position = requires_position;
                            requires_position += 1;
                            position
                        }
                        ContractProofFactKind::Ensures => {
                            let position = ensures_position;
                            ensures_position += 1;
                            position
                        }
                        ContractProofFactKind::Boundary => {
                            unreachable!("validated named contracts are requires or ensures")
                        }
                    };
                    let psi_typed_trees::domain::ProofFact::Proposition(application) =
                        program.proof_facts.get(fact)
                    else {
                        unreachable!("validated named signature contract must bind a proposition")
                    };
                    let normalized = program
                        .normalize_nominal_proposition_application(application)
                        .expect(
                            "validated named signature contract must have a nominal proposition endpoint",
                        );
                    let (evidence_type, evidence_interface) = match &normalized.classification {
                        psi_typed_trees::proposition::PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        } => (
                            evidence.clone(),
                            interface.as_ref().map(super::lower_checked_evidence_interface),
                        ),
                        psi_typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
                            unreachable!("validated named signature contract must bind witness evidence")
                        }
                    };
                    evidence_terms.append(CheckedEvidenceTerm {
                        name: binding.as_str().to_owned(),
                        owner,
                        kind,
                        lane_position,
                        proposition: super::lower_checked_proposition_application(normalized),
                        evidence_type,
                        evidence_interface,
                    })
                });
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
                    owner,
                    fact,
                    evidence_term,
                    qualification_authorization,
                });
            }
        }
    }
}
