use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::TransitionGuard;
use std::sync::Arc;

use super::segments::{segment_has_unconditional_transition, split_state_segments};
use super::transitions::plan_transition;
use super::{
    ContainedFlow, ControlFlowPlan, MachineFlow, PlannedTransitionTarget, StateFlow, TransitionFlow,
};

pub fn build_control_flow_plan(program: &Program) -> Result<ControlFlowPlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_control_flow_plan_with_workers(Arc::new(program.clone()), workers.handle())
}

pub fn build_control_flow_plan_with_workers(
    program: Arc<Program>,
    workers: WorkerPoolHandle,
) -> Result<ControlFlowPlan, Diagnostic> {
    if program.machines.is_empty() {
        return Ok(ControlFlowPlan::default());
    }

    let machine_count = program.machines.len();
    let machine_flows = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("control-flow worker index should be in range");
        let mut local_flow = ControlFlowPlan::default();
        let machine_flow = build_machine_flow(machine, &mut local_flow)?;

        Ok((local_flow, machine_flow))
    });

    let mut control_flow = ControlFlowPlan::default();
    for machine_flow in machine_flows {
        let (local_flow, machine_flow) = machine_flow?;

        merge_machine_flow(&mut control_flow, &local_flow, &machine_flow);
    }

    Ok(control_flow)
}

fn merge_machine_flow(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    machine_flow: &MachineFlow,
) {
    let states = source
        .states
        .span_or_empty(machine_flow.states)
        .iter()
        .map(|state| {
            let operations = target.operations.insert_many(
                source
                    .operations
                    .span_or_empty(state.operations)
                    .iter()
                    .cloned(),
            );
            let transitions = target.transitions.insert_many(
                source
                    .transitions
                    .span_or_empty(state.transitions)
                    .iter()
                    .cloned(),
            );

            StateFlow {
                operations,
                transitions,
                ..state.clone()
            }
        });
    let states = target.states.insert_many(states);

    target.machines.insert(MachineFlow {
        states,
        ..machine_flow.clone()
    });
}

fn build_machine_flow(
    machine: &Machine,
    control_flow: &mut ControlFlowPlan,
) -> Result<MachineFlow, Diagnostic> {
    let machine_symbol = machine.symbol;
    if !machine_symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "machine `{}` has no symbol",
            machine.name
        )));
    }
    let segments = machine
        .states
        .iter()
        .map(|state| split_state_segments(machine_symbol, state))
        .collect::<Vec<_>>();
    let state_indexes = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| (segment.name.as_str(), segment.key, index))
        .collect::<Vec<_>>();

    let states = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| {
            let mut transitions = segment
                .transitions
                .iter()
                .map(|transition| plan_transition(&state_indexes, transition))
                .collect::<Result<Vec<_>, Diagnostic>>()?;

            if let Some(next_segment_name) = &segment.next_segment_name
                && !segment_has_unconditional_transition(segment)
            {
                let next_index = state_indexes
                    .iter()
                    .find(|(name, _, _)| name == next_segment_name)
                    .map(|(_, _, index)| *index)
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "internal control-flow segment `{next_segment_name}` was not indexed"
                        ))
                    })?;
                let next_key = state_indexes
                    .iter()
                    .find(|(name, _, _)| name == next_segment_name)
                    .map(|(_, key, _)| *key)
                    .unwrap_or_default();

                transitions.push(TransitionFlow {
                    target: PlannedTransitionTarget::State {
                        index: next_index,
                        key: next_key,
                        name: next_segment_name.clone(),
                        arguments: Vec::new(),
                    },
                    continuation: None,
                    guard: TransitionGuard::Always,
                });
            }

            let operations = control_flow
                .operations
                .insert_many(segment.operations.iter().cloned());
            let transitions = control_flow.transitions.insert_many(transitions);

            Ok(StateFlow {
                key: segment.key,
                name: segment.name.clone(),
                index,
                parameters: segment.parameters.clone(),
                operations,
                transitions,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let states = control_flow.states.insert_many(states);

    Ok(MachineFlow {
        symbol: machine_symbol,
        name: machine.name.clone(),
        contains: machine
            .contains
            .iter()
            .map(|contained| ContainedFlow {
                symbol: contained.symbol,
                name: contained.name.clone(),
                type_symbol: contained.type_symbol,
                type_name: contained.type_name.clone(),
            })
            .collect(),
        states,
    })
}
