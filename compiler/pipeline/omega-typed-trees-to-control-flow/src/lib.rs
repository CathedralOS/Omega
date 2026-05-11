use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_trees::Program;
use omega_typed_trees::expression::ExpressionHandle;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::TransitionGuard;
use std::sync::Arc;

mod segments;
mod transitions;

use crate::segments::{segment_has_unconditional_transition, split_state_segments};
use crate::transitions::plan_transition;
use omega_control_flow::{
    ContainedFlow, ControlFlowPlan, MachineFlow, Operation, OperationExpressionRefs,
    PlannedTransitionTarget, StateFlow, StateKey, TransitionExpressionRefs, TransitionFlow,
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
        let machine_flow = build_machine_flow(machine, &program, &mut local_flow)?;

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
    let states = append_remapped_states(target, source, machine_flow.states);

    target.machines.insert(MachineFlow {
        states,
        ..machine_flow.clone()
    });
}

fn append_remapped_states(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    states: HandleSpan<StateFlow>,
) -> HandleSpan<StateFlow> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for state in source.states.span_or_empty(states) {
        let operations = append_remapped_operations(target, source, state.operations);
        let transitions = append_remapped_transitions(target, source, state.transitions);
        let handle = target.states.append(StateFlow {
            operations,
            transitions,
            ..state.clone()
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("control-flow state span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_remapped_operations(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    operations: HandleSpan<Operation>,
) -> HandleSpan<Operation> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for operation in source.operations.span_or_empty(operations) {
        let operation = remap_operation(target, source, operation);
        let handle = target.operations.append(operation);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("control-flow operation span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn append_remapped_transitions(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    transitions: HandleSpan<TransitionFlow>,
) -> HandleSpan<TransitionFlow> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for transition in source.transitions.span_or_empty(transitions) {
        let transition = remap_transition(target, source, transition);
        let handle = target.transitions.append(transition);
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("control-flow transition span count overflow");
    }

    if count == 0 {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(start, count)
    }
}

fn build_machine_flow(
    machine: &Machine,
    program: &Program,
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
        .map(|state| split_state_segments(machine_symbol, state, program, control_flow))
        .collect::<Vec<_>>();
    let state_indexes = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| (segment.key, index))
        .collect::<Vec<_>>();

    let states = append_machine_states(control_flow, program, &segments, &state_indexes)?;

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

fn append_machine_states(
    control_flow: &mut ControlFlowPlan,
    program: &Program,
    segments: &[Vec<crate::segments::StateSegment<'_>>],
    state_indexes: &[(StateKey, usize)],
) -> Result<HandleSpan<StateFlow>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for (index, segment) in segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
    {
        let operations = control_flow
            .operations
            .insert_many(segment.operations.iter().cloned());
        let transitions =
            append_segment_transitions(control_flow, program, segment, state_indexes)?;
        let handle = control_flow.states.append(StateFlow {
            key: segment.key,
            name: segment.name.clone(),
            index,
            parameters: segment.parameters.clone(),
            operations,
            transitions,
        });
        if count == 0 {
            start = handle;
        }
        count = count
            .checked_add(1)
            .expect("control-flow state span count overflow");
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
}

fn append_segment_transitions(
    control_flow: &mut ControlFlowPlan,
    program: &Program,
    segment: &crate::segments::StateSegment<'_>,
    state_indexes: &[(StateKey, usize)],
) -> Result<HandleSpan<TransitionFlow>, Diagnostic> {
    let mut start = Handle::invalid();
    let mut count = 0u32;

    for transition in &segment.transitions {
        let transition = plan_transition(state_indexes, transition, program, control_flow)?;
        append_transition(control_flow, transition, &mut start, &mut count);
    }

    if let Some(next_segment_name) = &segment.next_segment_name
        && !segment_has_unconditional_transition(segment)
    {
        let next_segment_key = StateKey {
            segment_index: segment.key.segment_index + 1,
            ..segment.key
        };
        let (next_key, next_index) = state_indexes
            .iter()
            .find(|(key, _)| *key == next_segment_key)
            .copied()
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "internal control-flow segment `{next_segment_name}` was not indexed"
                ))
            })?;

        append_transition(
            control_flow,
            TransitionFlow {
                target: PlannedTransitionTarget::State {
                    index: next_index,
                    key: next_key,
                    name: next_segment_name.clone(),
                },
                continuation: None,
                guard: TransitionGuard::Always,
                expressions: TransitionExpressionRefs::default(),
            },
            &mut start,
            &mut count,
        );
    }

    if count == 0 {
        Ok(HandleSpan::empty())
    } else {
        Ok(HandleSpan::from_parts(start, count))
    }
}

fn append_transition(
    control_flow: &mut ControlFlowPlan,
    transition: TransitionFlow,
    start: &mut Handle<TransitionFlow>,
    count: &mut u32,
) {
    let handle = control_flow.transitions.append(transition);
    if *count == 0 {
        *start = handle;
    }
    *count = count
        .checked_add(1)
        .expect("control-flow transition span count overflow");
}

fn remap_operation(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    operation: &Operation,
) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: operation.kind.clone(),
        expressions: remap_operation_expression_refs(target, source, operation.expressions),
    }
}

fn remap_operation_expression_refs(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    expressions: OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        OperationExpressionRefs::Assignment { target: lhs, value } => {
            OperationExpressionRefs::Assignment {
                target: copy_expression(target, source, lhs),
                value: copy_expression(target, source, value),
            }
        }
        OperationExpressionRefs::Call { arguments } => OperationExpressionRefs::Call {
            arguments: copy_expression_span(target, source, arguments),
        },
        OperationExpressionRefs::Expression(expression) => {
            OperationExpressionRefs::Expression(copy_expression(target, source, expression))
        }
        OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    transition: &TransitionFlow,
) -> TransitionFlow {
    TransitionFlow {
        target: transition.target.clone(),
        continuation: transition.continuation.clone(),
        guard: transition.guard.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: copy_expression_span(
                target,
                source,
                transition.expressions.target_arguments,
            ),
            continuation_arguments: copy_expression_span(
                target,
                source,
                transition.expressions.continuation_arguments,
            ),
            guard: transition
                .expressions
                .guard
                .map(|guard| copy_expression(target, source, guard)),
        },
    }
}

fn copy_expression(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    target
        .expressions
        .copy_from(&source.expressions, expression)
}

fn copy_expression_span(
    target: &mut ControlFlowPlan,
    source: &ControlFlowPlan,
    expressions: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    target
        .expressions
        .copy_expression_handles_from(&source.expressions, expressions)
}
