use super::*;

pub(crate) fn estimated_contract_fact_capacity(program: &psi_typed_trees::TypedTrees) -> usize {
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
                .map(|signature| {
                    program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| contract.facts.len())
                        .sum::<usize>()
                })
                .sum::<usize>()
        }))
        .chain(program.machines().iter().map(|machine| {
            program
                .machine_type_parameters(machine)
                .iter()
                .filter_map(|parameter| match &parameter.kind {
                    psi_typed_trees::data::TypeParameterKind::Machine { contract } => program
                        .machine_parameter_contract_view(contract)
                        .map(psi_typed_trees::data::MachineParameterContractView::signature),
                    _ => None,
                })
                .map(|signature| {
                    program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| contract.facts.len())
                        .sum::<usize>()
                })
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    contract_facts: &mut psi_arena::Arena<ContractProofFact>,
) {
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        append_trait_contract_facts_for_machine(
            program,
            machine,
            trait_definition,
            contract_facts,
            &mut visited_traits,
        );
    }
}

fn append_trait_contract_facts_for_machine(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    contract_facts: &mut psi_arena::Arena<ContractProofFact>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for signature in program.trait_machine_signatures(trait_definition) {
        let Some((target_machine_symbol, target_state_symbol)) =
            trait_requirement_state_symbols(program, machine, signature)
        else {
            continue;
        };

        for contract in program.state_signature_contracts(signature) {
            let Some(kind) = super::super::contract_fact_kind(&contract.kind) else {
                continue;
            };
            for fact in super::super::fact_handles(contract.facts) {
                let qualification_authorization =
                    crate::qualification_evidence::boundary_qualification_authorization(
                        program,
                        trait_definition.symbol,
                        signature,
                        contract.kind.clone(),
                        fact,
                    );
                contract_facts.append(ContractProofFact {
                    kind,
                    owner: ContractProofFactOwner::MachineState {
                        machine_symbol: target_machine_symbol,
                        state_symbol: target_state_symbol,
                    },
                    fact,
                    evidence_term: None,
                    qualification_authorization,
                });
            }
        }
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };
        append_trait_contract_facts_for_machine(
            program,
            machine,
            required_trait,
            contract_facts,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn estimated_inherited_trait_contract_fact_capacity(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
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
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    trait_definition: &psi_typed_trees::trait_definition::TraitDefinition,
    visited_traits: &mut Vec<SymbolHandle>,
) -> usize {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return 0;
    }

    visited_traits.push(trait_definition.symbol);

    let direct = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| trait_requirement_state_symbols(program, machine, signature).is_some())
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

fn trait_requirement_state_symbols(
    program: &psi_typed_trees::TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    requirement: &psi_typed_trees::signature::StateSignature,
) -> Option<(SymbolHandle, SymbolHandle)> {
    trait_conformance_candidate_machines(program, machine)
        .into_iter()
        .find_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name == requirement.name)
                .map(|state| (candidate.symbol, state.symbol))
        })
}

fn trait_conformance_candidate_machines<'program>(
    program: &'program psi_typed_trees::TypedTrees,
    machine: &'program psi_typed_trees::machine::Machine,
) -> Vec<&'program psi_typed_trees::machine::Machine> {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return vec![machine];
    };

    let mut candidates = Vec::new();
    candidates.push(machine);
    candidates.extend(program.machines().iter().filter(|candidate| {
        !std::ptr::eq(*candidate, machine)
            && candidate.attached_data.as_ref() == Some(attached_data)
    }));
    candidates
}

fn trait_definition_by_symbol(
    program: &psi_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&psi_typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}
