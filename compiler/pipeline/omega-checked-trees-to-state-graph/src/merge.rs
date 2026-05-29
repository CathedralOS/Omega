use omega_checked_trees::expression::ExpressionTable;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{
    MachineGraph, Operation, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot, StateBoundaryEdge,
    StateContractCall, StateContractExit, StateContractFactRef, StateDropEvent, StateGraph,
    StateMoveEvent, StateNode, StateParameterNode, StateValueFact, TransitionEdge,
};

use crate::borrows::remap_state_borrow_summary;
use crate::boundaries::remap_state_boundary_summary;
use crate::contracts::remap_state_contract_summary;
use crate::ownership::remap_state_ownership_summary;
use crate::remap::{append_remapped_operations, append_remapped_transitions};
use crate::values::remap_state_value_summary;

pub(crate) fn merge_machine_graph(
    target: &mut StateGraph,
    source: StateGraph,
    machine_graph: MachineGraph,
) {
    let StateGraph {
        expressions,
        machines: _,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        proof_obligations: _,
        invariants: _,
        contract_fact_refs,
        contract_calls,
        contract_exits,
        values,
        boundary_edges,
        borrow_writable_roots,
        borrow_access_segments,
        borrow_argument_accesses,
        borrow_calls,
        borrow_loans,
        borrow_activations,
        borrow_weakenings,
        ownership_segments,
        move_events,
        drop_events,
        operations,
        transitions,
    } = source;

    let states = append_remapped_states(
        target,
        &expressions,
        states.into_span_items(machine_graph.states),
        &state_parameters,
        &contract_fact_refs,
        &contract_calls,
        &contract_exits,
        &values,
        &boundary_edges,
        &borrow_writable_roots,
        &borrow_access_segments,
        &borrow_argument_accesses,
        &borrow_calls,
        &borrow_loans,
        &borrow_activations,
        &borrow_weakenings,
        &ownership_segments,
        &move_events,
        &drop_events,
        &operations,
        &transitions,
    );

    let contains = target
        .contained_machines
        .insert_many(contained_machines.into_span_items(machine_graph.contains));

    let owned_data = target
        .machine_owned_data
        .insert_many(machine_owned_data.into_span_items(machine_graph.owned_data));

    target.machines.insert(MachineGraph {
        symbol: machine_graph.symbol,
        name: machine_graph.name,
        attached_data: machine_graph.attached_data,
        direct_effects: machine_graph.direct_effects,
        reached_effects: machine_graph.reached_effects,
        contains,
        owned_data,
        states,
    });
}

fn append_remapped_states(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    states: impl Iterator<Item = StateNode>,
    source_state_parameters: &Arena<StateParameterNode>,
    source_contract_fact_refs: &Arena<StateContractFactRef>,
    source_contract_calls: &Arena<StateContractCall>,
    source_contract_exits: &Arena<StateContractExit>,
    source_values: &Arena<StateValueFact>,
    source_boundary_edges: &Arena<StateBoundaryEdge>,
    source_borrow_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_borrow_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_borrow_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_borrow_calls: &Arena<StateBorrowCall>,
    source_borrow_loans: &Arena<StateBorrowLoan>,
    source_borrow_activations: &Arena<StateBorrowActivation>,
    source_borrow_weakenings: &Arena<StateBorrowWeakening>,
    source_ownership_segments: &Arena<omega_facts::PlaceSegment>,
    source_move_events: &Arena<StateMoveEvent>,
    source_drop_events: &Arena<StateDropEvent>,
    source_operations: &Arena<Operation>,
    source_transitions: &Arena<TransitionEdge>,
) -> HandleSpan<StateNode> {
    let mut remapped_states = HandleSpan::empty();

    for state in states {
        let parameters = target.state_parameters.insert_many(
            source_state_parameters
                .span_or_empty(state.parameters)
                .iter()
                .cloned(),
        );

        let operations = append_remapped_operations(
            target,
            source_expressions,
            source_operations,
            state.operations,
        );
        let transitions = append_remapped_transitions(
            target,
            source_expressions,
            source_transitions,
            state.transitions,
        );
        let contracts = remap_state_contract_summary(
            target,
            source_contract_fact_refs,
            source_contract_calls,
            source_contract_exits,
            &state.contracts,
        );
        let values = remap_state_value_summary(target, source_values, &state.values);
        let boundaries =
            remap_state_boundary_summary(target, source_boundary_edges, &state.boundaries);
        let borrow = remap_state_borrow_summary(
            target,
            source_borrow_writable_roots,
            source_borrow_access_segments,
            source_borrow_argument_accesses,
            source_borrow_calls,
            source_borrow_loans,
            source_borrow_activations,
            source_borrow_weakenings,
            &state.borrow,
        );
        let ownership = remap_state_ownership_summary(
            target,
            source_ownership_segments,
            source_move_events,
            source_drop_events,
            &state.ownership,
        );
        target.states.append_to_span(
            &mut remapped_states,
            StateNode {
                key: state.key,
                name: state.name,
                index: state.index,
                direct_effects: state.direct_effects,
                reached_effects: state.reached_effects,
                parameters,
                contracts,
                values,
                boundaries,
                borrow,
                ownership,
                operations,
                transitions,
            },
        );
    }

    remapped_states
}
