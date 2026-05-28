use omega_checked_trees::CheckedTrees;
use omega_checked_trees::expression::ExpressionTableCapacity;
use omega_checked_trees::machine::Machine;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_state_graph::{
    MachineGraph, Operation, PlannedTransitionTarget, StateBorrowActivation,
    StateBorrowArgumentAccess, StateBorrowCall, StateBorrowLoan, StateBorrowWeakening,
    StateBorrowWritableRoot, StateContractCall, StateContractExit, StateContractFactRef,
    StateGraph, StateNode, StateParameterNode, TransitionEdge, TransitionExpressionRefs,
};
use std::sync::Arc;

use crate::borrows::{remap_state_borrow_summary, state_borrow_summary};
use crate::capacity::{
    StateGraphCapacity, estimated_machine_segment_capacity, machine_statement_count,
};
use crate::contracts::{remap_state_contract_summary, state_contract_summary};
use crate::facts::{remap_invariants, remap_proof_obligations};
use crate::machine_metadata::{
    machine_contains, machine_effect_bits, machine_owned_data, state_effect_bits,
};
use crate::remap::{append_remapped_operations, append_remapped_transitions};
use crate::segments::{segment_has_unconditional_transition, split_state_segments};
use crate::transitions::plan_transition;

pub(crate) fn build_state_graph_with_workers(
    program: Arc<CheckedTrees>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    if program.machines().is_empty() {
        return Ok(StateGraph::with_capacity(
            ExpressionTableCapacity::default(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ));
    }

    let machine_count = program.machines().len();
    let graph_capacity = StateGraphCapacity::for_program(&program);
    let program_for_machines = Arc::clone(&program);
    let machine_graphs = workers.map_ordered(machine_count, move |index| {
        let machine = program_for_machines
            .machines()
            .get(index)
            .expect("state-graph worker index should be in range");
        let mut local_state_graph =
            StateGraphCapacity::for_machine(&program_for_machines, machine).into_state_graph();
        let machine_graph =
            build_machine_graph(machine, &program_for_machines, &mut local_state_graph)?;

        Ok((local_state_graph, machine_graph))
    });

    let mut state_graph = graph_capacity.into_state_graph();
    for machine_graph in machine_graphs {
        let (local_state_graph, machine_graph) = machine_graph?;
        merge_machine_graph(&mut state_graph, local_state_graph, machine_graph);
    }

    state_graph.proof_obligations = remap_proof_obligations(
        program.facts.proof.obligations.len(),
        program.facts.proof.obligations.iter().map(|(_, fact)| fact),
    );
    state_graph.invariants = remap_invariants(
        program.facts.invariants.definitions.len(),
        program
            .facts
            .invariants
            .definitions
            .iter()
            .map(|(_, fact)| fact),
    );

    Ok(state_graph)
}

fn merge_machine_graph(target: &mut StateGraph, source: StateGraph, machine_graph: MachineGraph) {
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
        borrow_writable_roots,
        borrow_access_segments,
        borrow_argument_accesses,
        borrow_calls,
        borrow_loans,
        borrow_activations,
        borrow_weakenings,
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
        &borrow_writable_roots,
        &borrow_access_segments,
        &borrow_argument_accesses,
        &borrow_calls,
        &borrow_loans,
        &borrow_activations,
        &borrow_weakenings,
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
    source_expressions: &omega_checked_trees::expression::ExpressionTable,
    states: impl Iterator<Item = StateNode>,
    source_state_parameters: &Arena<StateParameterNode>,
    source_contract_fact_refs: &Arena<StateContractFactRef>,
    source_contract_calls: &Arena<StateContractCall>,
    source_contract_exits: &Arena<StateContractExit>,
    source_borrow_writable_roots: &Arena<StateBorrowWritableRoot>,
    source_borrow_access_segments: &Arena<omega_facts::PlaceSegment>,
    source_borrow_argument_accesses: &Arena<StateBorrowArgumentAccess>,
    source_borrow_calls: &Arena<StateBorrowCall>,
    source_borrow_loans: &Arena<StateBorrowLoan>,
    source_borrow_activations: &Arena<StateBorrowActivation>,
    source_borrow_weakenings: &Arena<StateBorrowWeakening>,
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
                borrow,
                operations,
                transitions,
            },
        );
    }

    remapped_states
}

fn build_machine_graph(
    machine: &Machine,
    program: &CheckedTrees,
    state_graph: &mut StateGraph,
) -> Result<MachineGraph, Diagnostic> {
    let machine_symbol = machine.symbol;
    if !machine_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "machine `{}` has no symbol",
            machine.name
        )));
    }

    let mut segments = Vec::with_capacity(estimated_machine_segment_capacity(program, machine));
    let mut segment_transitions =
        omega_core::arena::Arena::with_capacity(machine_statement_count(program, machine));
    for state in program.machine_states(machine) {
        split_state_segments(
            machine,
            state,
            program,
            state_graph,
            &mut segment_transitions,
            &mut segments,
        );
    }

    let states = append_machine_states(state_graph, program, &segments, &segment_transitions)?;

    let (direct_effects, reached_effects) = machine_effect_bits(program, machine_symbol);

    Ok(MachineGraph {
        symbol: machine_symbol,
        name: machine.name.clone(),
        attached_data: machine.attached_data.clone(),
        direct_effects,
        reached_effects,
        contains: machine_contains(state_graph, program, machine),
        owned_data: machine_owned_data(state_graph, program, machine),
        states,
    })
}

fn append_machine_states(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segments: &[crate::segments::StateSegment],
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> Result<HandleSpan<StateNode>, Diagnostic> {
    let mut states = HandleSpan::empty();

    for (index, segment) in segments.iter().enumerate() {
        let (direct_effects, reached_effects) = state_effect_bits(program, segment.key.state);
        let transitions = append_segment_transitions(
            state_graph,
            program,
            segment,
            segments,
            segment_transitions,
        )?;
        let contracts = state_contract_summary(state_graph, program, segment, segment_transitions);
        let borrow = state_borrow_summary(state_graph, program, segment.key);
        state_graph.states.append_to_span(
            &mut states,
            StateNode {
                key: segment.key,
                name: segment.name.clone(),
                index,
                direct_effects,
                reached_effects,
                parameters: segment.parameters,
                contracts,
                borrow,
                operations: segment.operations,
                transitions,
            },
        );
    }

    Ok(states)
}

fn append_segment_transitions(
    state_graph: &mut StateGraph,
    program: &CheckedTrees,
    segment: &crate::segments::StateSegment,
    segments: &[crate::segments::StateSegment],
    segment_transitions: &omega_core::arena::Arena<crate::segments::SegmentTransition>,
) -> Result<HandleSpan<TransitionEdge>, Diagnostic> {
    let mut transitions = HandleSpan::empty();

    for transition in segment_transitions.span_or_empty(segment.transitions) {
        let transition = plan_transition(segment.key, segments, transition, program, state_graph)?;
        state_graph
            .transitions
            .append_to_span(&mut transitions, transition);
    }

    if segment.next_segment_key.is_valid()
        && !segment_has_unconditional_transition(segment, segment_transitions)
    {
        let next_segment_key = segment.next_segment_key;
        let (next_index, next_segment) = segments
            .iter()
            .enumerate()
            .find(|(_, segment)| segment.key == next_segment_key)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "internal state-graph segment #{} was not indexed",
                    next_segment_key.segment_index
                ))
            })?;

        state_graph.transitions.append_to_span(
            &mut transitions,
            TransitionEdge {
                statement_index: 0,
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_segment.key,
                    name: next_segment.name.clone(),
                },
                continuation: PlannedTransitionTarget::None,
                expressions: TransitionExpressionRefs::default(),
            },
        );
    }

    Ok(transitions)
}
