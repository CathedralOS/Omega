use crate::proof::proposition_vocabulary::{
    contract_fact_kind, fact_handles, lower_checked_evidence_interface,
    lower_checked_proposition_application,
};
use checked_trees::CheckedEvidenceTerm;
use checked_trees::{ContractProofFact, ContractProofFactKind, ContractProofFactOwner};
use symbols::SymbolHandle;
pub(crate) mod call_signatures;
pub(crate) mod calls;
mod inherited;
mod operators;

pub(crate) use calls::{
    build_contract_call_facts, build_contract_exit_facts, contract_target_from_state_symbol,
};
pub(crate) use inherited::{
    append_inherited_trait_contract_facts, estimated_contract_fact_capacity,
};
pub(crate) use operators::build_contract_operator_use_facts;

/// The named-evidence lanes of one contract owner's clause list.
///
/// A lane position counts that owner's named bindings of that kind in authored
/// clause order. Erased call lanes and review-evidence projection both address
/// evidence terms by it, so a published term and its position must always
/// agree; `requires` and `ensures` count independently.
#[derive(Default)]
pub(crate) struct ContractEvidenceLanes {
    requires: usize,
    ensures: usize,
}

impl ContractEvidenceLanes {
    pub(crate) fn next(&mut self, kind: ContractProofFactKind) -> usize {
        let counter = match kind {
            ContractProofFactKind::Requires => &mut self.requires,
            ContractProofFactKind::Ensures => &mut self.ensures,
        };
        let position = *counter;
        *counter += 1;
        position
    }
}

/// Publish the checked evidence term one named contract binding owns.
///
/// Validation guarantees the binding names a proposition whose nominal
/// endpoint carries witness evidence; a row that lost that endpoint is
/// reported here rather than re-derived.
pub(crate) fn append_named_contract_evidence_term(
    program: &dyn ::validation::CheckingInput,
    owner: ContractProofFactOwner,
    kind: ContractProofFactKind,
    lane_position: usize,
    fact: arena::Handle<typed_trees::domain::ProofFact>,
    binding: &str,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<arena::Handle<CheckedEvidenceTerm>, diagnostics::Diagnostic> {
    let Some(application) = program.proof_facts.get(fact).as_proposition() else {
        return Err(diagnostics::Diagnostic::error(format!(
            "named contract binding `{binding}` is not a validated proposition"
        )));
    };
    let Some(normalized) = program.normalize_nominal_proposition_application(application, None)
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "named contract binding `{binding}` lost its nominal proposition endpoint"
        )));
    };
    let typed_trees::proposition::PropositionEvidenceClassification::Witness {
        evidence,
        interface,
    } = &normalized.classification
    else {
        return Err(diagnostics::Diagnostic::error(format!(
            "named contract binding `{binding}` must bind witness evidence"
        )));
    };
    let evidence_type = evidence.clone();
    let evidence_interface = interface.as_ref().map(lower_checked_evidence_interface);
    Ok(evidence_terms.append(CheckedEvidenceTerm {
        name: binding.to_owned(),
        owner,
        kind,
        lane_position,
        proposition: lower_checked_proposition_application(normalized),
        evidence_type,
        evidence_interface,
    }))
}

/// Publish the checked facts one owner's contract clause list owes.
///
/// Every clause names its own kind; a named binding additionally publishes its
/// checked evidence term in this list's lane order. `boundary_signature` is
/// the owner's boundary-authorization policy: a public logical signature
/// carries one, machine and state contract rows carry none.
fn append_contract_clause_facts(
    program: &dyn ::validation::CheckingInput,
    contracts: &[typed_trees::signature::SignatureContract],
    owner: ContractProofFactOwner,
    boundary_signature: Option<(SymbolHandle, &typed_trees::signature::StateSignature)>,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    let mut lanes = ContractEvidenceLanes::default();
    for contract in contracts {
        let Some(kind) = contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in fact_handles(contract.facts) {
            let evidence_term = match contract.binding.as_ref() {
                Some(binding) => Some(append_named_contract_evidence_term(
                    program,
                    owner,
                    kind,
                    lanes.next(kind),
                    fact,
                    binding.as_str(),
                    evidence_terms,
                )?),
                None => None,
            };
            let qualification_authorization =
                boundary_signature.and_then(|(owner_symbol, signature)| {
                    ::validation::boundary_qualification_authorization(
                        program,
                        owner_symbol,
                        signature,
                        contract.kind.clone(),
                        program.proof_facts.get(fact),
                    )
                });
            contract_facts.append(ContractProofFact {
                kind,
                owner,
                fact,
                evidence_term,
                qualification_authorization,
                inherited_scope: None,
            });
        }
    }
    Ok(())
}

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
    program: &'program dyn ::validation::CheckingInput,
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

/// Publish a machine's own contract clause facts.
pub(crate) fn append_machine_contract_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    append_contract_clause_facts(
        program,
        program.machine_contracts(machine),
        ContractProofFactOwner::Machine {
            machine_symbol: machine.symbol,
        },
        None,
        contract_facts,
        evidence_terms,
    )
}

pub(crate) fn append_operator_declaration_contract_facts(
    program: &dyn ::validation::CheckingInput,
    operator: &typed_trees::operator::OperatorDefinition,
    contract_facts: &mut arena::Arena<ContractProofFact>,
) {
    let owner = ContractProofFactOwner::OperatorDeclaration {
        operator_symbol: operator.symbol,
    };
    for contract in program.operator_contracts(operator) {
        let Some(kind) = contract_fact_kind(&contract.kind) else {
            continue;
        };
        for fact in fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind,
                owner,
                fact,
                evidence_term: None,
                qualification_authorization: None,
                inherited_scope: None,
            });
        }
    }
}

/// Publish a state's arrival contract clause facts.
///
/// The syntax stage admits `requires` only on states, so a state clause list
/// carries no exit guarantees to number.
pub(crate) fn append_state_contract_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    append_contract_clause_facts(
        program,
        program.state_contracts(state),
        ContractProofFactOwner::MachineState {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
        },
        None,
        contract_facts,
        evidence_terms,
    )
}

/// Append the contract facts owed by every evidence signature reachable
/// from `parameters` through machine-parameter evidence.
pub(crate) fn append_parameter_evidence_contract_facts(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::data::TypeParameter],
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    for (owner_symbol, _, contract) in machine_parameter_evidence_signatures(program, parameters) {
        append_state_signature_contract_facts(
            program,
            owner_symbol,
            std::slice::from_ref(contract),
            contract_facts,
            evidence_terms,
        )?;
    }
    Ok(())
}

pub(crate) fn append_state_signature_contract_facts(
    program: &dyn ::validation::CheckingInput,
    owner_symbol: SymbolHandle,
    signatures: &[typed_trees::signature::StateSignature],
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    for signature in signatures {
        append_signature_contract_facts(
            program,
            owner_symbol,
            signature,
            ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol: signature.symbol,
            },
            contract_facts,
            evidence_terms,
        )?;
    }
    Ok(())
}

/// One builder retains the entire logical signature and both ordered evidence
/// lanes. An invocation-qualified owner changes identity, never its clauses.
pub(crate) fn append_signature_contract_facts(
    program: &dyn ::validation::CheckingInput,
    owner_symbol: SymbolHandle,
    signature: &typed_trees::signature::StateSignature,
    owner: ContractProofFactOwner,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    evidence_terms: &mut arena::Arena<CheckedEvidenceTerm>,
) -> Result<(), diagnostics::Diagnostic> {
    append_contract_clause_facts(
        program,
        program.state_signature_contracts(signature),
        owner,
        Some((owner_symbol, signature)),
        contract_facts,
        evidence_terms,
    )
}
