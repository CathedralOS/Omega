use omega_state_graph::{
    MachineGraph, Operation, StateBoundaryEdge, StateGraph, StateGraphCode,
    StateGraphSemanticRoots, StateNode, StateParameterNode, StateValueFact, TransitionEdge,
};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::ExpressionTable;

use crate::borrows::{SourceBorrowArenas, remap_state_borrow_summary};
use crate::boundaries::remap_state_boundary_summary;
use crate::contracts::{SourceContractArenas, remap_state_contract_summary};
use crate::ownership::{SourceOwnershipArenas, remap_state_ownership_summary};
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
        service_reach: _,
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
            contracts: SourceContractArenas {
                fact_refs: &contracts.fact_refs,
                calls: &contracts.calls,
                exits: &contracts.exits,
            },
            values: &values.values,
            boundaries: &boundaries.edges,
            borrow: SourceBorrowArenas {
                writable_roots: &borrow.writable_roots,
                access_segments: &borrow.access_segments,
                argument_accesses: &borrow.argument_accesses,
                calls: &borrow.calls,
                loans: &borrow.loans,
                activations: &borrow.activations,
                weakenings: &borrow.weakenings,
            },
            ownership: SourceOwnershipArenas {
                segments: &ownership.segments,
                permissions: &ownership.permissions,
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
        service_reach: machine_graph.service_reach,
        suspension: machine_graph.suspension,
        blocking: machine_graph.blocking,
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
    contracts: SourceContractArenas<'a>,
    values: &'a Arena<StateValueFact>,
    boundaries: &'a Arena<StateBoundaryEdge>,
    borrow: SourceBorrowArenas<'a>,
    ownership: SourceOwnershipArenas<'a>,
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
        let contracts =
            remap_state_contract_summary(target, &source.semantics.contracts, &state.contracts);
        let values = remap_state_value_summary(target, source.semantics.values, &state.values);
        let boundaries =
            remap_state_boundary_summary(target, source.semantics.boundaries, &state.boundaries);
        let borrow = remap_state_borrow_summary(target, &source.semantics.borrow, &state.borrow);
        let ownership =
            remap_state_ownership_summary(target, &source.semantics.ownership, &state.ownership);
        target.states.append_to_span(
            &mut remapped_states,
            StateNode {
                key: state.key,
                name: state.name,
                index: state.index,
                service_reach: state.service_reach,
                suspension: state.suspension,
                blocking: state.blocking,
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
