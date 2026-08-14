use omega_state_graph::{ContainedGraph, MachineOwnedDataGraph, StateGraph};
use psi_arena::HandleSpan;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_language_semantics::{BlockingSummary, ServiceReachSummary, SuspensionSummary};

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
    machine_symbol: psi_symbols::SymbolHandle,
) -> ServiceReachSummary {
    program
        .facts
        .service_reaches
        .for_machine(machine_symbol)
        .map(|reach| ServiceReachSummary {
            direct: reach.inferred_direct,
            transitive: reach.inferred_transitive,
        })
        .unwrap_or_default()
}

pub(crate) fn state_service_reach(
    program: &CheckedTrees,
    state_symbol: psi_symbols::SymbolHandle,
) -> ServiceReachSummary {
    program
        .facts
        .service_reaches
        .for_state(state_symbol)
        .map(|reach| ServiceReachSummary {
            direct: reach.inferred_direct,
            transitive: reach.inferred_transitive,
        })
        .unwrap_or_default()
}

pub(crate) fn machine_suspension_summary(
    program: &CheckedTrees,
    machine_symbol: psi_symbols::SymbolHandle,
) -> SuspensionSummary {
    let Some(machine) = program
        .facts
        .operational
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
    else {
        return SuspensionSummary::default();
    };
    let mut direct_may_suspend = false;
    for state in program
        .facts
        .operational
        .states
        .span_or_empty(machine.states)
    {
        direct_may_suspend |= state.direct_may_suspend;
        for call in program.facts.operational.calls.span_or_empty(state.calls) {
            direct_may_suspend |= call.direct_may_suspend;
        }
    }
    SuspensionSummary {
        direct_may_suspend,
        transitive_may_suspend: machine.transitive_may_suspend,
    }
}

pub(crate) fn machine_blocking_summary(
    program: &CheckedTrees,
    machine_symbol: psi_symbols::SymbolHandle,
) -> BlockingSummary {
    let Some(machine) = program
        .facts
        .operational
        .machines()
        .iter()
        .find(|effects| effects.symbol == machine_symbol)
    else {
        return BlockingSummary::default();
    };
    let mut direct_may_block = false;
    for state in program
        .facts
        .operational
        .states
        .span_or_empty(machine.states)
    {
        direct_may_block |= state.direct_may_block;
        for call in program.facts.operational.calls.span_or_empty(state.calls) {
            direct_may_block |= call.direct_may_block;
        }
    }
    BlockingSummary {
        direct_may_block,
        transitive_may_block: machine.transitive_may_block,
    }
}

pub(crate) fn state_suspension_summary(
    program: &CheckedTrees,
    state_symbol: psi_symbols::SymbolHandle,
) -> SuspensionSummary {
    let Some(state) = program
        .facts
        .operational
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .facts
                .operational
                .states
                .span_or_empty(machine.states)
        })
        .find(|effects| effects.symbol == state_symbol)
    else {
        return SuspensionSummary::default();
    };
    let mut direct_may_suspend = state.direct_may_suspend;
    for call in program.facts.operational.calls.span_or_empty(state.calls) {
        direct_may_suspend |= call.direct_may_suspend;
    }
    SuspensionSummary {
        direct_may_suspend,
        transitive_may_suspend: state.transitive_may_suspend,
    }
}

pub(crate) fn state_blocking_summary(
    program: &CheckedTrees,
    state_symbol: psi_symbols::SymbolHandle,
) -> BlockingSummary {
    let Some(state) = program
        .facts
        .operational
        .machines()
        .iter()
        .flat_map(|machine| {
            program
                .facts
                .operational
                .states
                .span_or_empty(machine.states)
        })
        .find(|effects| effects.symbol == state_symbol)
    else {
        return BlockingSummary::default();
    };
    let mut direct_may_block = state.direct_may_block;
    for call in program.facts.operational.calls.span_or_empty(state.calls) {
        direct_may_block |= call.direct_may_block;
    }
    BlockingSummary {
        direct_may_block,
        transitive_may_block: state.transitive_may_block,
    }
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
