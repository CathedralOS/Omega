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
        boundaries,
        borrow,
        ownership,
    } = semantics;

    let source_arenas = SourceStateArenas {
        code: SourceStateCodeArenas {
            expressions: &expressions,
            state_parameters: &state_parameters,
            operations: &operations,
            transitions: &transitions,
        },
        semantics: SourceStateSemanticArenas {
            contracts: SourceStateContractArenas {
                fact_refs: &contracts.fact_refs,
                calls: &contracts.calls,
                exits: &contracts.exits,
            },
            values: &values.values,
            boundaries: &boundaries.edges,
            borrow: SourceStateBorrowArenas {
                writable_roots: &borrow.writable_roots,
                access_segments: &borrow.access_segments,
                argument_accesses: &borrow.argument_accesses,
                calls: &borrow.calls,
                loans: &borrow.loans,
                activations: &borrow.activations,
                weakenings: &borrow.weakenings,
            },
            ownership: SourceStateOwnershipArenas {
                segments: &ownership.segments,
                moves: &ownership.moves,
                drops: &ownership.drops,
            },
        },
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
    code: SourceStateCodeArenas<'a>,
    semantics: SourceStateSemanticArenas<'a>,
}

struct SourceStateCodeArenas<'a> {
    expressions: &'a ExpressionTable,
    state_parameters: &'a Arena<StateParameterNode>,
    operations: &'a Arena<Operation>,
    transitions: &'a Arena<TransitionEdge>,
}

struct SourceStateSemanticArenas<'a> {
    contracts: SourceStateContractArenas<'a>,
    values: &'a Arena<StateValueFact>,
    boundaries: &'a Arena<StateBoundaryEdge>,
    borrow: SourceStateBorrowArenas<'a>,
    ownership: SourceStateOwnershipArenas<'a>,
}

struct SourceStateContractArenas<'a> {
    fact_refs: &'a Arena<StateContractFactRef>,
    calls: &'a Arena<StateContractCall>,
    exits: &'a Arena<StateContractExit>,
}

struct SourceStateBorrowArenas<'a> {
    writable_roots: &'a Arena<StateBorrowWritableRoot>,
    access_segments: &'a Arena<omega_facts::PlaceSegment>,
    argument_accesses: &'a Arena<StateBorrowArgumentAccess>,
    calls: &'a Arena<StateBorrowCall>,
    loans: &'a Arena<StateBorrowLoan>,
    activations: &'a Arena<StateBorrowActivation>,
    weakenings: &'a Arena<StateBorrowWeakening>,
}

struct SourceStateOwnershipArenas<'a> {
    segments: &'a Arena<omega_facts::PlaceSegment>,
    moves: &'a Arena<StateMoveEvent>,
    drops: &'a Arena<StateDropEvent>,
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
                .code
                .state_parameters
                .span_or_empty(state.parameters)
                .iter()
                .cloned(),
        );

        let operations = append_remapped_operations(
            target,
            source.code.expressions,
            source.code.operations,
            state.operations,
        );
        let transitions = append_remapped_transitions(
            target,
            source.code.expressions,
            source.code.transitions,
            state.transitions,
        );
        let contracts = remap_state_contract_summary(
            target,
            source.semantics.contracts.fact_refs,
            source.semantics.contracts.calls,
            source.semantics.contracts.exits,
            &state.contracts,
        );
        let values = remap_state_value_summary(target, source.semantics.values, &state.values);
        let boundaries =
            remap_state_boundary_summary(target, source.semantics.boundaries, &state.boundaries);
        let borrow = remap_state_borrow_summary(
            target,
            source.semantics.borrow.writable_roots,
            source.semantics.borrow.access_segments,
            source.semantics.borrow.argument_accesses,
            source.semantics.borrow.calls,
            source.semantics.borrow.loans,
            source.semantics.borrow.activations,
            source.semantics.borrow.weakenings,
            &state.borrow,
        );
        let ownership = remap_state_ownership_summary(
            target,
            source.semantics.ownership.segments,
            source.semantics.ownership.moves,
            source.semantics.ownership.drops,
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
