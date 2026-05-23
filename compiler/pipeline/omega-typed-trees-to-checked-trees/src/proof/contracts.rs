use super::*;
use crate::lookup::machine_state_count;

pub(crate) fn estimated_contract_fact_capacity(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_contracts(machine)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
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
        .chain(program.platforms().iter().map(|platform| {
            program
                .platform_state_signatures(platform)
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
        .chain(
            program
                .machines()
                .iter()
                .map(|machine| estimated_inherited_trait_contract_fact_capacity(program, machine)),
        )
        .sum()
}

pub(crate) fn append_machine_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    for contract in program.machine_contracts(machine) {
        for fact in super::fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind: super::contract_fact_kind(contract.kind),
                owner: ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                fact,
            });
        }
    }
}

pub(crate) fn append_state_signature_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    owner_symbol: SymbolHandle,
    signatures: &[omega_typed_trees::signature::StateSignature],
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    for signature in signatures {
        for contract in program.state_signature_contracts(signature) {
            for fact in super::fact_handles(contract.facts) {
                contract_facts.append(ContractProofFact {
                    kind: super::contract_fact_kind(contract.kind),
                    owner: ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol: signature.symbol,
                    },
                    fact,
                });
            }
        }
    }
}

pub(crate) fn append_inherited_trait_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
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
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
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
            for fact in super::fact_handles(contract.facts) {
                contract_facts.append(ContractProofFact {
                    kind: super::contract_fact_kind(contract.kind),
                    owner: ContractProofFactOwner::MachineState {
                        machine_symbol: target_machine_symbol,
                        state_symbol: target_state_symbol,
                    },
                    fact,
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
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
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
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
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
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    requirement: &omega_typed_trees::signature::StateSignature,
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
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
) -> Vec<&'program omega_typed_trees::machine::Machine> {
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
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}

pub(crate) fn build_contract_call_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
) -> (
    omega_core::arena::Arena<ContractProofFactRef>,
    omega_core::arena::Arena<ContractCallFact>,
) {
    let mut fact_refs = omega_core::arena::Arena::with_capacity(contract_facts.len());
    let mut calls = omega_core::arena::Arena::with_capacity(borrow.calls.len());

    for state in borrow.states.iter().map(|(_, state)| state) {
        for call in borrow.calls.span_or_empty(state.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                contract_target_from_state_symbol(program, call.target_symbol)
            else {
                continue;
            };

            append_contract_call(
                contract_facts,
                &mut fact_refs,
                &mut calls,
                ContractCallSite {
                    caller_machine_symbol: state.machine_symbol,
                    caller_state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine_symbol,
                    target_state_symbol,
                },
            );
        }
    }

    (fact_refs, calls)
}

#[derive(Debug, Clone, Copy)]
struct ContractCallSite {
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
}

fn append_contract_call(
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
    calls: &mut omega_core::arena::Arena<ContractCallFact>,
    site: ContractCallSite,
) {
    let requires = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Requires,
    );
    let ensures = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Ensures,
    );

    if requires.is_empty() && ensures.is_empty() {
        return;
    }

    calls.append(ContractCallFact {
        caller_machine_symbol: site.caller_machine_symbol,
        caller_state_symbol: site.caller_state_symbol,
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        target_machine_symbol: site.target_machine_symbol,
        target_state_symbol: site.target_state_symbol,
        requires,
        ensures,
    });
}

pub(crate) fn build_contract_exit_facts(
    program: &omega_typed_trees::TypedTrees,
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
) -> omega_core::arena::Arena<ContractExitFact> {
    let mut exits = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let Some((statement_index, StatementNode::Expression(_))) =
                statements.iter().enumerate().next_back()
            else {
                continue;
            };
            let ensures = append_contract_fact_refs(
                contract_facts,
                fact_refs,
                machine.symbol,
                Some(state.symbol),
                ContractProofFactKind::Ensures,
            );

            if ensures.is_empty() {
                continue;
            }

            exits.append(ContractExitFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                ensures,
            });
        }
    }

    exits
}

fn append_contract_fact_refs(
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
    machine_symbol: SymbolHandle,
    state_symbol: Option<SymbolHandle>,
    kind: ContractProofFactKind,
) -> HandleSpan<ContractProofFactRef> {
    let mut span = HandleSpan::empty();

    for (handle, fact) in contract_facts.iter() {
        let owner_matches = match fact.owner {
            ContractProofFactOwner::Machine {
                machine_symbol: owner_symbol,
            } => owner_symbol == machine_symbol,
            ContractProofFactOwner::MachineState {
                machine_symbol: owner_machine_symbol,
                state_symbol: owner_state_symbol,
            } => {
                owner_machine_symbol == machine_symbol
                    && state_symbol.is_some_and(|state_symbol| state_symbol == owner_state_symbol)
            }
            ContractProofFactOwner::Unknown | ContractProofFactOwner::StateSignature { .. } => {
                false
            }
        };

        if owner_matches && fact.kind == kind {
            fact_refs.append_to_span(&mut span, ContractProofFactRef { fact: handle });
        }
    }

    span
}

pub(crate) fn contract_target_from_state_symbol(
    program: &omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if !target_state_symbol.is_valid() {
        return None;
    }

    let target_machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state_symbol)
    })?;
    Some((target_machine.symbol, target_state_symbol))
}
