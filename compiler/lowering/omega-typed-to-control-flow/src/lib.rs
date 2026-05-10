use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use omega_typed_program::expression::ExpressionHandle;
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::TransitionGuard;
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
    let states = source
        .states
        .span_or_empty(machine_flow.states)
        .iter()
        .map(|state| {
            let operations = source
                .operations
                .span_or_empty(state.operations)
                .iter()
                .map(|operation| remap_operation(target, source, operation))
                .collect::<Vec<_>>();
            let operations = target.operations.insert_many(operations);
            let transitions = source
                .transitions
                .span_or_empty(state.transitions)
                .iter()
                .map(|transition| remap_transition(target, source, transition))
                .collect::<Vec<_>>();
            let transitions = target.transitions.insert_many(transitions);

            StateFlow {
                operations,
                transitions,
                ..state.clone()
            }
        })
        .collect::<Vec<_>>();
    let states = target.states.insert_many(states);

    target.machines.insert(MachineFlow {
        states,
        ..machine_flow.clone()
    });
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

    let states = segments
        .iter()
        .flat_map(|state_segments| state_segments.iter())
        .enumerate()
        .map(|(index, segment)| {
            let mut transitions = segment
                .transitions
                .iter()
                .map(|transition| {
                    plan_transition(&state_indexes, transition, program, control_flow)
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;

            if let Some(next_segment_name) = &segment.next_segment_name
                && !segment_has_unconditional_transition(segment)
            {
                let next_segment_key = StateKey {
                    segment_index: segment.key.segment_index + 1,
                    ..segment.key
                };
                let next_index = state_indexes
                    .iter()
                    .find(|(key, _)| *key == next_segment_key)
                    .map(|(_, index)| *index)
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "internal control-flow segment `{next_segment_name}` was not indexed"
                        ))
                    })?;
                let next_key = state_indexes
                    .iter()
                    .find(|(key, _)| *key == next_segment_key)
                    .map(|(key, _)| *key)
                    .unwrap_or_default();

                transitions.push(TransitionFlow {
                    target: PlannedTransitionTarget::State {
                        index: next_index,
                        key: next_key,
                        name: next_segment_name.clone(),
                    },
                    continuation: None,
                    guard: TransitionGuard::Always,
                    expressions: TransitionExpressionRefs::default(),
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
