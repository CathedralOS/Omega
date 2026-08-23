use omega_control_flow::{ContainedFlow, MachineFlow, MachineOwnedDataFlow};
use omega_state_graph::{ContainedGraph, MachineGraph, MachineOwnedDataGraph, StateGraph};
use psi_arena::Arena;

use crate::handles::{remap_contained_span, remap_owned_data_span, remap_state_span};

pub(crate) fn remap_machines(
    state_graph: &StateGraph,
) -> (
    Arena<MachineFlow>,
    Arena<ContainedFlow>,
    Arena<MachineOwnedDataFlow>,
) {
    let mut machines = Arena::with_capacity(state_graph.machines.len());
    let mut contained_machines = Arena::with_capacity(state_graph.contained_machines.len());
    let mut machine_owned_data = Arena::with_capacity(state_graph.machine_owned_data.len());

    for (_, machine) in state_graph.machines.iter() {
        machines.append(remap_machine(
            state_graph,
            machine,
            &mut contained_machines,
            &mut machine_owned_data,
        ));
    }

    (machines, contained_machines, machine_owned_data)
}

pub(crate) fn remap_machine_owned(machine: MachineGraph) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name,
        attached_data: machine.attached_data,
        service_reach: machine.service_reach,
        suspension: machine.suspension,
        blocking: machine.blocking,
        contains: remap_contained_span(machine.contains),
        owned_data: remap_owned_data_span(machine.owned_data),
        states: remap_state_span(machine.states),
    }
}

pub(crate) fn remap_contained_owned(contained: ContainedGraph) -> ContainedFlow {
    ContainedFlow {
        symbol: contained.symbol,
        name: contained.name,
        type_symbol: contained.type_symbol,
        type_name: contained.type_name,
    }
}

pub(crate) fn remap_owned_data_owned(data: MachineOwnedDataGraph) -> MachineOwnedDataFlow {
    MachineOwnedDataFlow {
        symbol: data.symbol,
        name: data.name,
        type_reference: data.type_reference,
    }
}

fn remap_machine(
    state_graph: &StateGraph,
    machine: &MachineGraph,
    contained_machines: &mut Arena<ContainedFlow>,
    machine_owned_data: &mut Arena<MachineOwnedDataFlow>,
) -> MachineFlow {
    MachineFlow {
        symbol: machine.symbol,
        name: machine.name.clone(),
        attached_data: machine.attached_data.clone(),
        service_reach: machine.service_reach,
        suspension: machine.suspension,
        blocking: machine.blocking,
        contains: contained_machines.insert_many(
            state_graph
                .machine_contains(machine)
                .iter()
                .map(remap_contained),
        ),
        owned_data: machine_owned_data.insert_many(
            state_graph
                .machine_owned_data(machine)
                .iter()
                .map(remap_owned_data),
        ),
        states: remap_state_span(machine.states),
    }
}

fn remap_contained(contained: &ContainedGraph) -> ContainedFlow {
    ContainedFlow {
        symbol: contained.symbol,
        name: contained.name.clone(),
        type_symbol: contained.type_symbol,
        type_name: contained.type_name.clone(),
    }
}

fn remap_owned_data(data: &MachineOwnedDataGraph) -> MachineOwnedDataFlow {
    MachineOwnedDataFlow {
        symbol: data.symbol,
        name: data.name.clone(),
        type_reference: data.type_reference,
    }
}
