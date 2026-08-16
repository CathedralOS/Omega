use omega_core::parallel::WorkerPoolHandle;
use omega_state_graph::{MachineGraph, StateGraph, StateGraphServiceReachRoots};
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::machine::Machine;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

use crate::capacity::{
    StateGraphCapacity, estimated_machine_segment_capacity, machine_statement_count,
};
use crate::dynamic_conformances::validated_dynamic_conformance_bindings;
use crate::facts::{remap_invariants, remap_proof_obligations};
use crate::machine_metadata::{
    machine_blocking_summary, machine_contains, machine_owned_data, machine_service_reach,
    machine_suspension_summary,
};
use crate::merge::merge_machine_graph;
use crate::segments::split_state_segments;
use crate::states::append_machine_states;

pub(crate) fn build_state_graph_with_workers(
    program: Arc<CheckedTrees>,
    workers: WorkerPoolHandle,
) -> Result<StateGraph, Diagnostic> {
    if program.machines().is_empty() {
        let mut state_graph = StateGraph::default();
        install_service_reach_roots(&mut state_graph, &program);
        return Ok(state_graph);
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
    install_service_reach_roots(&mut state_graph, &program);
    for machine_graph in machine_graphs {
        let (local_state_graph, machine_graph) = machine_graph?;
        merge_machine_graph(&mut state_graph, local_state_graph, machine_graph);
    }

    state_graph.semantics.facts.proof_obligations = remap_proof_obligations(&program)?;
    state_graph.semantics.facts.invariants = remap_invariants(
        program.facts.invariants.definitions.len(),
        program
            .facts
            .invariants
            .definitions
            .iter()
            .map(|(_, fact)| fact),
    );
    state_graph.semantics.facts.dynamic_conformances =
        validated_dynamic_conformance_bindings(&program)?;

    Ok(state_graph)
}

fn install_service_reach_roots(state_graph: &mut StateGraph, program: &CheckedTrees) {
    let reach = &program.facts.service_reaches;
    state_graph.semantics.service_reach =
        StateGraphServiceReachRoots::with_roots(reach.services.clone(), reach.rows.clone());
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
        psi_arena::Arena::with_capacity(machine_statement_count(program, machine));
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

    let states = append_machine_states(
        state_graph,
        program,
        machine,
        &segments,
        &segment_transitions,
    )?;

    Ok(MachineGraph {
        symbol: machine_symbol,
        name: machine.name.clone(),
        attached_data: machine.attached_data.clone(),
        service_reach: machine_service_reach(program, machine_symbol),
        suspension: machine_suspension_summary(program, machine_symbol),
        blocking: machine_blocking_summary(program, machine_symbol),
        contains: machine_contains(state_graph, program, machine),
        owned_data: machine_owned_data(state_graph, program, machine),
        states,
    })
}
