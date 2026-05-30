use omega_checked_trees::expression::ExpressionTable;
use omega_core::arena::{Arena, HandleSpan};
use omega_state_graph::{
    MachineGraph, Operation, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot, StateBoundaryEdge,
    StateContractCall, StateContractExit, StateContractFactRef, StateDropEvent, StateGraph,
    StateGraphCode, StateGraphSemanticRoots, StateMoveEvent, StateNode, StateParameterNode,
    StateValueFact, TransitionEdge,
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
    let StateGraph { code, semantics } = source;
    let StateGraphCode {
        expressions,
        machines: _,
        contained_machines,
        machine_owned_data,
        states,
        state_parameters,
        operations,
        transitions,
    } = code;
    let StateGraphSemanticRoots {
        facts: _,
        contracts,
        values,
        boundary_edges,
        borrow,
        ownership,
    } = semantics;

    let source_arenas = SourceStateArenas {
        expressions: &expressions,
        state_parameters: &state_parameters,
        contract_fact_refs: &contracts.fact_refs,
        contract_calls: &contracts.calls,
        contract_exits: &contracts.exits,
        values: &values,
        boundary_edges: &boundary_edges,
        borrow_writable_roots: &borrow.writable_roots,
        borrow_access_segments: &borrow.access_segments,
        borrow_argument_accesses: &borrow.argument_accesses,
        borrow_calls: &borrow.calls,
        borrow_loans: &borrow.loans,
        borrow_activations: &borrow.activations,
        borrow_weakenings: &borrow.weakenings,
        ownership_segments: &ownership.segments,
        move_events: &ownership.moves,
        drop_events: &ownership.drops,
        operations: &operations,
        transitions: &transitions,
    };
    let states = append_remapped_states(
        target,
        &source_arenas,
        states.into_span_items(machine_graph.states),
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

struct SourceStateArenas<'a> {
    expressions: &'a ExpressionTable,
    state_parameters: &'a Arena<StateParameterNode>,
    contract_fact_refs: &'a Arena<StateContractFactRef>,
    contract_calls: &'a Arena<StateContractCall>,
    contract_exits: &'a Arena<StateContractExit>,
    values: &'a Arena<StateValueFact>,
    boundary_edges: &'a Arena<StateBoundaryEdge>,
    borrow_writable_roots: &'a Arena<StateBorrowWritableRoot>,
    borrow_access_segments: &'a Arena<omega_facts::PlaceSegment>,
    borrow_argument_accesses: &'a Arena<StateBorrowArgumentAccess>,
    borrow_calls: &'a Arena<StateBorrowCall>,
    borrow_loans: &'a Arena<StateBorrowLoan>,
    borrow_activations: &'a Arena<StateBorrowActivation>,
    borrow_weakenings: &'a Arena<StateBorrowWeakening>,
    ownership_segments: &'a Arena<omega_facts::PlaceSegment>,
    move_events: &'a Arena<StateMoveEvent>,
    drop_events: &'a Arena<StateDropEvent>,
    operations: &'a Arena<Operation>,
    transitions: &'a Arena<TransitionEdge>,
}

fn append_remapped_states(
    target: &mut StateGraph,
    source: &SourceStateArenas<'_>,
    states: impl Iterator<Item = StateNode>,
) -> HandleSpan<StateNode> {
    let mut remapped_states = HandleSpan::empty();

    for state in states {
        let parameters = target.state_parameters.insert_many(
            source
                .state_parameters
                .span_or_empty(state.parameters)
                .iter()
                .cloned(),
        );

        let operations = append_remapped_operations(
            target,
            source.expressions,
            source.operations,
            state.operations,
        );
        let transitions = append_remapped_transitions(
            target,
            source.expressions,
            source.transitions,
            state.transitions,
        );
        let contracts = remap_state_contract_summary(
            target,
            source.contract_fact_refs,
            source.contract_calls,
            source.contract_exits,
            &state.contracts,
        );
        let values = remap_state_value_summary(target, source.values, &state.values);
        let boundaries =
            remap_state_boundary_summary(target, source.boundary_edges, &state.boundaries);
        let borrow = remap_state_borrow_summary(
            target,
            source.borrow_writable_roots,
            source.borrow_access_segments,
            source.borrow_argument_accesses,
            source.borrow_calls,
            source.borrow_loans,
            source.borrow_activations,
            source.borrow_weakenings,
            &state.borrow,
        );
        let ownership = remap_state_ownership_summary(
            target,
            source.ownership_segments,
            source.move_events,
            source.drop_events,
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
