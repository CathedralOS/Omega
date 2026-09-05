use super::*;
use checked_trees::CheckedEvidenceTerm;
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

/// Collect the callable signatures that need binder-owned checked evidence.
///
/// Top-level machine parameters retain the historical behavior for both
/// structural and nominal contracts. Nested parameters are different: an
/// inline structural contract owns another binder contract and therefore
/// needs its own evidence rows, while a nominal contract merely references a
/// trait requirement whose evidence is emitted with that trait.
///
/// The traversal is iterative and deduplicates arena-backed parameter
/// identities. Besides avoiding call-stack growth, this makes a malformed
/// cyclic signature span terminate without repeatedly emitting the same row.
pub(crate) fn machine_parameter_evidence_signatures<'program>(
    program: &'program typed_trees::TypedTrees,
    parameters: &'program [typed_trees::data::TypeParameter],
) -> Vec<(
    SymbolHandle,
    SymbolHandle,
    &'program typed_trees::signature::StateSignature,
)> {
    let mut signatures = Vec::new();
    let mut pending = parameters
        .iter()
        .rev()
        .map(|parameter| (parameter, true))
        .collect::<Vec<_>>();
    let mut visited = std::collections::HashSet::new();

    while let Some((parameter, is_top_level)) = pending.pop() {
        if !visited.insert(parameter as *const typed_trees::data::TypeParameter) {
            continue;
        }
        let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
            continue;
        };
        match contract {
            // Trait-level requirement-identity binders are declaration
            // parameters, not executable machine contracts.
            typed_trees::data::MachineParameterContract::RequirementIdentity => {}
            typed_trees::data::MachineParameterContract::Structural(signature) => {
                let target_state = if is_top_level {
                    parameter.symbol
                } else {
                    signature.symbol
                };
                signatures.push((parameter.symbol, target_state, signature));
                pending.extend(
                    program
                        .state_signature_type_parameters(signature)
                        .iter()
                        .rev()
                        .map(|nested| (nested, false)),
                );
            }
            typed_trees::data::MachineParameterContract::Nominal { .. } if is_top_level => {
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect(
                        "typed machine-parameter contract must retain a valid requirement identity",
                    )
                    .signature();
                signatures.push((parameter.symbol, parameter.symbol, signature));
            }
            typed_trees::data::MachineParameterContract::Nominal { .. } => {}
        }
    }

    signatures
}

pub(crate) fn append_machine_contract_facts(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
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
                };
                let typed_trees::domain::ProofFact::Proposition(application) =
                    program.proof_facts.get(fact)
                else {
                    unreachable!("validated named contract must bind a proposition")
                };
                let normalized = program
                    .normalize_nominal_proposition_application(application)
                    .expect("validated named contract must have a nominal proposition endpoint");
                let (evidence_type, evidence_interface) = match &normalized.classification {
                    typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => (
                        evidence.clone(),
                        interface
                            .as_ref()
                            .map(super::lower_checked_evidence_interface),
                    ),
                    typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
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

pub(crate) fn append_operator_declaration_contract_facts(
    program: &typed_trees::TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
    contract_facts: &mut arena::Arena<ContractProofFact>,
) {
    let owner = ContractProofFactOwner::OperatorDeclaration {
        operator_symbol: operator.symbol,
    };
    for contract in program.operator_contracts(operator) {
        let Some(kind) = super::contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in super::fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind,
                owner,
                fact,
                evidence_term: None,
                qualification_authorization: None,
            });
        }
    }
}

pub(crate) fn append_state_contract_facts(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) {
    let mut requires_position = 0usize;
    for contract in program.state_contracts(state) {
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
                        unreachable!("states admit only arrival requires contracts")
                    }
                };
                let typed_trees::domain::ProofFact::Proposition(application) =
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
                    typed_trees::proposition::PropositionEvidenceClassification::Witness {
                        evidence,
                        interface,
                    } => (
                        evidence.clone(),
                        interface
                            .as_ref()
                            .map(super::lower_checked_evidence_interface),
                    ),
                    typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
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
    program: &typed_trees::TypedTrees,
    owner_symbol: SymbolHandle,
    signatures: &[typed_trees::signature::StateSignature],
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
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
                    };
                    let typed_trees::domain::ProofFact::Proposition(application) =
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
                        typed_trees::proposition::PropositionEvidenceClassification::Witness {
                            evidence,
                            interface,
                        } => (
                            evidence.clone(),
                            interface.as_ref().map(super::lower_checked_evidence_interface),
                        ),
                        typed_trees::proposition::PropositionEvidenceClassification::FactOnly => {
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
