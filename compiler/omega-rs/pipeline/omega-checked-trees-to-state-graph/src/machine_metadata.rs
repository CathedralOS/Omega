use omega_state_graph::{ContainedGraph, MachineOwnedDataGraph, StateGraph};
use psi_arena::HandleSpan;
use psi_checked_trees::machine::Machine;
use psi_checked_trees::state::State;
use psi_checked_trees::{
    CheckedTrees, FlowStateFact, MachineServiceReachRows, StateServiceReachRows,
};
use psi_language_semantics::{
    BlockingSummary, ServiceReachRowId, ServiceReachRowTable, ServiceReachSummary,
    SuspensionSummary,
};
use psi_symbols::SymbolHandle;

fn exact_typed_machine(program: &CheckedTrees, symbol: SymbolHandle) -> &Machine {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches.next().unwrap_or_else(|| {
        panic!("state-graph metadata invariant: exact typed machine is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact typed machine is duplicated"
    );
    machine
}

fn exact_typed_state_owner(program: &CheckedTrees, symbol: SymbolHandle) -> (&Machine, &State) {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(move |state| state.symbol == symbol)
            .map(move |state| (machine, state))
    });
    let owner = matches
        .next()
        .unwrap_or_else(|| panic!("state-graph metadata invariant: exact typed state is missing"));
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact typed state is duplicated"
    );
    owner
}

fn exact_machine_service_reach(
    program: &CheckedTrees,
    machine: SymbolHandle,
) -> &MachineServiceReachRows {
    let reaches = &program.facts.service_reaches;
    let mut matches = reaches
        .machines()
        .iter()
        .filter(|reach| reach.machine == machine);
    let reach = matches.next().unwrap_or_else(|| {
        panic!("state-graph metadata invariant: exact machine service-reach row is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact machine service-reach row is duplicated"
    );
    reach
}

fn exact_state_service_reach<'a>(
    program: &'a CheckedTrees,
    machine_reach: &MachineServiceReachRows,
    state: SymbolHandle,
) -> &'a StateServiceReachRows {
    let reaches = &program.facts.service_reaches;
    let mut matches = reaches
        .states_for(machine_reach)
        .iter()
        .filter(|reach| reach.state == state);
    let Some(reach) = matches.next() else {
        assert!(
            !reaches
                .machines()
                .iter()
                .filter(|candidate| candidate.machine != machine_reach.machine)
                .flat_map(|candidate| reaches.states_for(candidate))
                .any(|candidate| candidate.state == state),
            "state-graph metadata invariant: exact state service-reach row belongs to another machine"
        );
        panic!("state-graph metadata invariant: exact state service-reach row is missing");
    };
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact state service-reach row is duplicated"
    );
    assert!(
        !reaches
            .machines()
            .iter()
            .filter(|candidate| candidate.machine != machine_reach.machine)
            .flat_map(|candidate| reaches.states_for(candidate))
            .any(|candidate| candidate.state == state),
        "state-graph metadata invariant: exact state service-reach row belongs to another machine"
    );
    reach
}

fn validate_service_row(program: &CheckedTrees, row: ServiceReachRowId) {
    let reaches = &program.facts.service_reaches;
    let services = reaches.rows.services(row);
    assert!(
        !services.is_empty() || row == ServiceReachRowTable::EMPTY_ROW,
        "state-graph metadata invariant: service-reach row identity is noncanonical"
    );
    assert!(
        services
            .iter()
            .all(|service| reaches.services.definition(*service).is_some()),
        "state-graph metadata invariant: service-reach row contains an unregistered service"
    );
}

fn exact_flow_state(
    program: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> &FlowStateFact {
    let mut matches = program
        .facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, fact)| fact)
        .filter(|fact| fact.state_symbol == state);
    let fact = matches.next().unwrap_or_else(|| {
        panic!("state-graph metadata invariant: exact FlowState fact is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact FlowState fact is duplicated"
    );
    assert_eq!(
        fact.machine_symbol, machine,
        "state-graph metadata invariant: exact FlowState fact belongs to another machine"
    );
    fact
}

fn exact_suspension(
    program: &CheckedTrees,
    machine: SymbolHandle,
) -> psi_language_semantics::SuspensionPlan {
    let mut matches = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let fact = matches.next().unwrap_or_else(|| {
        panic!("state-graph metadata invariant: exact machine suspension row is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact machine suspension row is duplicated"
    );
    fact.plan
}

fn exact_blocking(
    program: &CheckedTrees,
    machine: SymbolHandle,
) -> psi_language_semantics::BlockingPlan {
    let mut matches = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine);
    let fact = matches.next().unwrap_or_else(|| {
        panic!("state-graph metadata invariant: exact machine blocking row is missing")
    });
    assert!(
        matches.next().is_none(),
        "state-graph metadata invariant: exact machine blocking row is duplicated"
    );
    fact.plan
}

pub(crate) fn machine_owned_data(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
) -> HandleSpan<MachineOwnedDataGraph> {
    state_graph
        .machine_owned_data
        .insert_many(
            program
                .machine_owned_data(machine)
                .iter()
                .map(|data| MachineOwnedDataGraph {
                    symbol: data.symbol,
                    name: data.name.clone(),
                    type_reference: data.type_reference,
                }),
        )
}

pub(crate) fn machine_service_reach(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
) -> ServiceReachSummary {
    let _ = exact_typed_machine(program, machine_symbol);
    let reach = exact_machine_service_reach(program, machine_symbol);
    validate_service_row(program, reach.inferred_direct);
    validate_service_row(program, reach.inferred_transitive);
    ServiceReachSummary {
        direct: reach.inferred_direct,
        transitive: reach.inferred_transitive,
    }
}

pub(crate) fn state_service_reach(
    program: &CheckedTrees,
    state_symbol: SymbolHandle,
) -> ServiceReachSummary {
    let (machine, _) = exact_typed_state_owner(program, state_symbol);
    let machine_reach = exact_machine_service_reach(program, machine.symbol);
    let reach = exact_state_service_reach(program, machine_reach, state_symbol);
    validate_service_row(program, reach.inferred_direct);
    validate_service_row(program, reach.inferred_transitive);
    ServiceReachSummary {
        direct: reach.inferred_direct,
        transitive: reach.inferred_transitive,
    }
}

pub(crate) fn machine_suspension_summary(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
) -> SuspensionSummary {
    let machine = exact_typed_machine(program, machine_symbol);
    let states = program.machine_states(machine);
    for (index, state) in states.iter().enumerate() {
        assert!(
            !states[..index]
                .iter()
                .any(|candidate| candidate.symbol == state.symbol),
            "state-graph metadata invariant: typed machine has a duplicate state identity"
        );
    }
    let flow_states = states
        .iter()
        .map(|state| exact_flow_state(program, machine_symbol, state.symbol))
        .collect::<Vec<_>>();
    let direct_may_suspend = flow_states
        .iter()
        .any(|state| state.suspension.direct_may_suspend);
    let transitive_may_suspend = exact_suspension(program, machine_symbol).checked_may_suspend;
    SuspensionSummary {
        direct_may_suspend,
        transitive_may_suspend,
    }
}

pub(crate) fn machine_blocking_summary(
    program: &CheckedTrees,
    machine_symbol: SymbolHandle,
) -> BlockingSummary {
    let machine = exact_typed_machine(program, machine_symbol);
    let states = program.machine_states(machine);
    for (index, state) in states.iter().enumerate() {
        assert!(
            !states[..index]
                .iter()
                .any(|candidate| candidate.symbol == state.symbol),
            "state-graph metadata invariant: typed machine has a duplicate state identity"
        );
    }
    let flow_states = states
        .iter()
        .map(|state| exact_flow_state(program, machine_symbol, state.symbol))
        .collect::<Vec<_>>();
    let direct_may_block = flow_states
        .iter()
        .any(|state| state.blocking.direct_may_block);
    let transitive_may_block = exact_blocking(program, machine_symbol).checked_may_block;
    BlockingSummary {
        direct_may_block,
        transitive_may_block,
    }
}

pub(crate) fn state_suspension_summary(
    program: &CheckedTrees,
    state_symbol: SymbolHandle,
) -> SuspensionSummary {
    let (machine, _) = exact_typed_state_owner(program, state_symbol);
    exact_flow_state(program, machine.symbol, state_symbol).suspension
}

pub(crate) fn state_blocking_summary(
    program: &CheckedTrees,
    state_symbol: SymbolHandle,
) -> BlockingSummary {
    let (machine, _) = exact_typed_state_owner(program, state_symbol);
    exact_flow_state(program, machine.symbol, state_symbol).blocking
}

pub(crate) fn machine_contains(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    machine: &Machine,
) -> HandleSpan<ContainedGraph> {
    let mut contains = HandleSpan::empty();
    for contained in program
        .facts
        .carry
        .contained_fields_for_machine(machine.symbol)
    {
        let Some(field) = program.data_definitions().iter().find_map(|definition| {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    psi_checked_trees::data::DataMember::Field(field)
                        if field.symbol == contained.field =>
                    {
                        Some(field)
                    }
                    _ => None,
                })
        }) else {
            continue;
        };
        let Some(field_data) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == contained.data)
        else {
            continue;
        };
        let Some(target) = program
            .facts
            .carry
            .contained_targets_for_field(contained)
            .first()
        else {
            continue;
        };

        let contained_symbol = program
            .symbols
            .find_child_by_name(machine.symbol, field.name.as_str())
            .unwrap_or(field.symbol);

        state_graph.contained_machines.append_to_span(
            &mut contains,
            ContainedGraph {
                symbol: contained_symbol,
                name: field.name.clone(),
                type_symbol: target.machine,
                type_name: field_data.name.clone(),
            },
        );
    }

    contains
}

#[cfg(test)]
mod tests;
