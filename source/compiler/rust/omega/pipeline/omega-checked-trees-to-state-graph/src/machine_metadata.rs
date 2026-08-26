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
    let machine = exact_typed_machine(program, machine.symbol);
    let carry = &program.facts.carry;
    let mut matching_topologies = carry
        .machine_topologies
        .iter()
        .map(|(_, topology)| topology)
        .filter(|topology| topology.machine == machine.symbol);
    let topology = matching_topologies.next().unwrap_or_else(|| {
        panic!("state-graph topology invariant: exact machine topology row is missing")
    });
    assert!(
        matching_topologies.next().is_none(),
        "state-graph topology invariant: exact machine topology row is duplicated"
    );
    let fields = carry.contained_fields.span_or_empty(topology.fields);
    assert!(
        topology.fields.is_empty() || !fields.is_empty(),
        "state-graph topology invariant: contained field span is invalid"
    );

    let attached_data = if fields.is_empty() {
        None
    } else {
        let attached_name = machine.attached_data.as_ref().unwrap_or_else(|| {
            panic!("state-graph topology invariant: contained fields have no attached data owner")
        });
        let mut owners = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.name == *attached_name);
        let owner = owners.next().unwrap_or_else(|| {
            panic!("state-graph topology invariant: attached data owner is missing")
        });
        assert!(
            owners.next().is_none(),
            "state-graph topology invariant: attached data owner is duplicated"
        );
        Some(owner)
    };

    let mut contains = HandleSpan::empty();
    for (index, contained) in fields.iter().enumerate() {
        assert!(
            contained.field.is_valid()
                && !fields[..index]
                    .iter()
                    .any(|candidate| candidate.field == contained.field),
            "state-graph topology invariant: contained field identity is empty or duplicated"
        );

        let mut field_owners = program.data_definitions().iter().flat_map(|definition| {
            program
                .data_members(definition)
                .iter()
                .filter_map(move |member| match member {
                    psi_checked_trees::data::DataMember::Field(field)
                        if field.symbol == contained.field =>
                    {
                        Some((definition, field))
                    }
                    _ => None,
                })
        });
        let (field_owner, field) = field_owners.next().unwrap_or_else(|| {
            panic!("state-graph topology invariant: contained field is missing")
        });
        assert!(
            field_owners.next().is_none(),
            "state-graph topology invariant: contained field identity is cross-owned or duplicated"
        );
        assert_eq!(
            Some(field_owner.symbol),
            attached_data.map(|owner| owner.symbol),
            "state-graph topology invariant: contained field belongs to another data owner"
        );
        assert_eq!(
            contained.type_reference, field.type_reference,
            "state-graph topology invariant: contained field type-reference coordinate drifted"
        );
        assert_eq!(
            program.type_reference_symbol(contained.type_reference),
            contained.data,
            "state-graph topology invariant: contained field data coordinate drifted"
        );

        let mut field_data_matches = program
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == contained.data);
        let field_data = field_data_matches.next().unwrap_or_else(|| {
            panic!("state-graph topology invariant: contained field data definition is missing")
        });
        assert!(
            field_data_matches.next().is_none(),
            "state-graph topology invariant: contained field data definition is duplicated"
        );

        let targets = carry.contained_targets.span_or_empty(contained.targets);
        assert!(
            !contained.targets.is_empty() && !targets.is_empty(),
            "state-graph topology invariant: contained target span is empty or invalid"
        );
        for (target_index, target) in targets.iter().enumerate() {
            assert!(
                target.machine.is_valid()
                    && !targets[..target_index]
                        .iter()
                        .any(|candidate| candidate.machine == target.machine),
                "state-graph topology invariant: contained target identity is empty or duplicated"
            );
            let target_machine = exact_typed_machine(program, target.machine);
            assert_eq!(
                target_machine.attached_data.as_ref(),
                Some(&field_data.name),
                "state-graph topology invariant: contained target is attached to another data definition"
            );
        }
        let target = &targets[0];

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
