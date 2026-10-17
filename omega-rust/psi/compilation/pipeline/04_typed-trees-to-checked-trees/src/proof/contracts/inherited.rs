use super::CheckedEvidenceTerm;
use checked_trees::{ContractProofFact, ContractProofFactKind, ContractProofFactOwner};
use symbols::SymbolHandle;

/// Facts carried by one state signature's contracts.
fn signature_fact_total(
    program: &dyn ::validation::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
) -> usize {
    program
        .state_signature_contracts(signature)
        .iter()
        .map(|contract| contract.facts.len())
        .sum()
}

pub(crate) fn estimated_contract_fact_capacity(program: &dyn ::validation::CheckingInput) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            let machine_facts = program
                .machine_contracts(machine)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>();
            let state_facts = program
                .machine_states(machine)
                .iter()
                .flat_map(|state| program.state_contracts(state))
                .map(|contract| contract.facts.len())
                .sum::<usize>();
            machine_facts + state_facts
        })
        .chain(program.traits().iter().map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .map(|signature| signature_fact_total(program, signature))
                .sum::<usize>()
        }))
        .chain(program.all_operators().map(|operator| {
            program
                .operator_contracts(operator)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
        }))
        .chain(program.machines().iter().map(|machine| {
            program
                .machine_type_parameters(machine)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    typed_trees::data::TypeParameterKind::Machine { contract } => program
                        .machine_parameter_contract_view(contract)
                        .map(typed_trees::data::MachineParameterContractView::signature),
                    _ => None,
                })
                .map(|signature| signature_fact_total(program, signature))
                .sum::<usize>()
        }))
        .chain(
            program
                .machines()
                .iter()
                .map(|machine| estimated_inherited_trait_contract_fact_capacity(program, machine)),
        )
        .sum()
}

pub(crate) fn append_inherited_trait_contract_facts(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    contract_facts: &mut arena::Arena<ContractProofFact>,
    inherited_scopes: &mut arena::Arena<checked_trees::InheritedContractScope>,
    evidence_terms: &arena::Arena<CheckedEvidenceTerm>,
) {
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        let effective_arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        append_trait_contract_facts_for_machine(
            program,
            machine,
            trait_definition,
            effective_arguments,
            contract_facts,
            inherited_scopes,
            evidence_terms,
            &mut visited_traits,
        );
    }
}

fn append_trait_contract_facts_for_machine(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    effective_arguments: &[typed_trees::types::TypeReferenceHandle],
    contract_facts: &mut arena::Arena<ContractProofFact>,
    inherited_scopes: &mut arena::Arena<checked_trees::InheritedContractScope>,
    evidence_terms: &arena::Arena<CheckedEvidenceTerm>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits.contains(&trait_definition.symbol) {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for signature in program.trait_machine_signatures(trait_definition) {
        let exact_direct_requirement =
            machine_selects_exact_requirement(program, machine, trait_definition.symbol, signature);
        let Some((target_machine_symbol, target_state_symbol)) =
            trait_requirement_state_symbols(program, machine, trait_definition.symbol, signature)
        else {
            continue;
        };

        let mut requires_position = 0usize;
        let mut ensures_position = 0usize;
        for contract in program.state_signature_contracts(signature) {
            let Some(kind) =
                crate::proof::proposition_vocabulary::contract_fact_kind(&contract.kind)
            else {
                continue;
            };
            for fact in crate::proof::proposition_vocabulary::fact_handles(contract.facts) {
                let evidence_term = contract.binding.as_ref().and_then(|_| {
                    if !exact_direct_requirement {
                        return None;
                    }
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
                    evidence_terms.iter().find_map(|(handle, term)| {
                        (term.owner
                            == ContractProofFactOwner::Machine {
                                machine_symbol: target_machine_symbol,
                            }
                            && term.kind == kind
                            && term.lane_position == lane_position)
                            .then_some(handle)
                    })
                });
                let qualification_authorization =
                    ::validation::boundary_qualification_authorization(
                        program,
                        trait_definition.symbol,
                        signature,
                        contract.kind.clone(),
                        program.proof_facts.get(fact),
                    );
                // The retained `fact` handle keeps the requirement's authored
                // row; the scope records the exact edge arguments so semantic
                // labels and obligations can instantiate the schema instead of
                // guessing from display names.
                let scope = inherited_scopes.append(checked_trees::InheritedContractScope {
                    conformance_machine: machine.symbol,
                    declaring_trait: trait_definition.symbol,
                    requirement: signature.symbol,
                    trait_arguments: effective_arguments.to_vec(),
                });
                contract_facts.append(ContractProofFact {
                    kind,
                    owner: ContractProofFactOwner::MachineState {
                        machine_symbol: target_machine_symbol,
                        state_symbol: target_state_symbol,
                    },
                    fact,
                    evidence_term,
                    qualification_authorization,
                    inherited_scope: Some(scope),
                });
            }
        }
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };
        // Parent requirements are written against this trait's own parameter
        // telescope (for example `Reflexive<C, Relation>` inside
        // `Equivalence<C, proposition Relation>`), so compose the authored
        // parent arguments with this edge's concrete arguments before the
        // parent's contract facts migrate further.
        let parent_arguments = program
            .type_reference_table
            .type_reference_handles(requirement.arguments);
        let composed_arguments = validation::compose_forwarded_trait_arguments(
            program,
            trait_definition,
            effective_arguments,
            parent_arguments,
        );
        append_trait_contract_facts_for_machine(
            program,
            machine,
            required_trait,
            &composed_arguments,
            contract_facts,
            inherited_scopes,
            evidence_terms,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn estimated_inherited_trait_contract_fact_capacity(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> usize {
    let mut visited_traits = Vec::new();
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| trait_definition_by_symbol(program, conformance.symbol))
        .map(|trait_definition| {
            estimated_trait_contract_fact_capacity_for_machine(
                program,
                machine,
                trait_definition,
                &mut visited_traits,
            )
        })
        .sum()
}

fn estimated_trait_contract_fact_capacity_for_machine(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    trait_definition: &typed_trees::trait_definition::TraitDefinition,
    visited_traits: &mut Vec<SymbolHandle>,
) -> usize {
    if visited_traits.contains(&trait_definition.symbol) {
        return 0;
    }

    visited_traits.push(trait_definition.symbol);

    let direct = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| {
            trait_requirement_state_symbols(program, machine, trait_definition.symbol, signature)
                .is_some()
        })
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
        })
        .sum::<usize>();
    let inherited = program
        .trait_requirements(trait_definition)
        .iter()
        .filter_map(|requirement| trait_definition_by_symbol(program, requirement.symbol))
        .map(|required_trait| {
            estimated_trait_contract_fact_capacity_for_machine(
                program,
                machine,
                required_trait,
                visited_traits,
            )
        })
        .sum::<usize>();

    visited_traits.pop();
    direct.saturating_add(inherited)
}

/// The state that owes `requirement`'s contract facts through `machine`'s
/// own conformance row. Every row names one exact requirement, so each
/// implementing machine appends its own requirement's facts when its turn
/// comes; a sibling requirement of the same trait is that sibling's row to
/// publish. Appending it here as well, located through a same-named sibling
/// state, restated one requirement once per conforming machine of the
/// attached type (ten rows for one clause on a ten-leaf provider) under a
/// conformance edge the sibling never authored.
fn trait_requirement_state_symbols(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    trait_symbol: SymbolHandle,
    requirement: &typed_trees::signature::StateSignature,
) -> Option<(SymbolHandle, SymbolHandle)> {
    machine_selects_exact_requirement(program, machine, trait_symbol, requirement)
        .then(|| {
            program
                .machine_states(machine)
                .first()
                .map(|state| (machine.symbol, state.symbol))
        })
        .flatten()
}

fn machine_selects_exact_requirement(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    trait_symbol: SymbolHandle,
    requirement: &typed_trees::signature::StateSignature,
) -> bool {
    program
        .machine_trait_conformances(machine)
        .iter()
        .any(|conformance| {
            conformance.symbol == trait_symbol
                && conformance
                    .requirement
                    .as_ref()
                    .is_some_and(|name| *name == requirement.name)
        })
}

fn trait_definition_by_symbol(
    program: &dyn ::validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program.trait_definition_by_symbol(symbol)
}
