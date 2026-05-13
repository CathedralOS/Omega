use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardLowering, StateGuardOperator, lower_guard_conjunction};
use omega_checked_trees::expression::{BinaryOperator, Expression};
use omega_checked_trees::statement::TransitionGuard;
use omega_state_schedule::ScheduledState;

use super::{EmissionBlocker, blocker};

pub(super) fn runtime_and_required_states(
    input: &EmissionPlanningInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in input.runtime_flow.states.iter() {
        push_scheduled_state_key(&mut states, state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state_key(&mut states, state_call.source_key);

            if state_call.target_key.is_valid() {
                push_scheduled_state_key(&mut states, state_call.target_key);
            }
        }
    }

    states
}

fn push_scheduled_state_key(states: &mut Vec<ScheduledState>, key: omega_control_flow::StateKey) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(ScheduledState { key });
}

fn state_name(input: &EmissionPlanningInput<'_>, key: omega_control_flow::StateKey) -> String {
    input
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}

pub(super) fn collect_runtime_dispatch_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, cycle) in input.runtime_flow.cycles.iter() {
        let Some(states) = input.runtime_flow.cycle_states.span(cycle.states) else {
            blockers.insert(blocker(
                "runtime dispatch",
                "invalid runtime cycle span in native flow plan",
            ));
            continue;
        };
        let cycle_path = states
            .iter()
            .map(|state| state_name(input, state.key))
            .collect::<Vec<_>>()
            .join(" -> ");

        blockers.insert(blocker(
            "runtime dispatch",
            &format!("cycle {cycle_path} needs generated state dispatch before native emission"),
        ));
    }
}

pub(super) fn runtime_dispatch_loop_blocker(input: &EmissionPlanningInput<'_>) -> EmissionBlocker {
    if let Some(guard_lowering) = first_unsupported_dispatch_guard(input) {
        return blocker(
            "runtime dispatch",
            &format!(
                "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); guard lowering {guard_lowering:?} needs runtime state comparison byte emission",
                input.runtime_dispatch_loop.cases.len(),
                input.runtime_dispatch_loop.edges.len(),
                input.runtime_flow.cycles.len()
            ),
        );
    }

    blocker(
        "runtime dispatch",
        &format!(
            "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); native emission needs dispatch loop byte emission",
            input.runtime_dispatch_loop.cases.len(),
            input.runtime_dispatch_loop.edges.len(),
            input.runtime_flow.cycles.len()
        ),
    )
}

pub(super) fn runtime_dispatch_loop_can_emit(input: &EmissionPlanningInput<'_>) -> bool {
    input.runtime_dispatch_loop.cases.iter().all(|(_, case)| {
        input
            .runtime_dispatch_loop
            .edges
            .span(case.edges)
            .unwrap_or(&[])
            .iter()
            .all(|edge| {
                (dispatch_loop_guard_can_emit(edge)
                    || decomposed_guard_can_emit(input, case.key, case.dispatch_index, edge))
                    && edge.action != RuntimeDispatchLoopAction::Unknown
            })
    })
}

fn first_unsupported_dispatch_guard(
    input: &EmissionPlanningInput<'_>,
) -> Option<StateGuardLowering> {
    input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| {
            input
                .runtime_dispatch_loop
                .edges
                .span(case.edges)
                .unwrap_or(&[])
                .iter()
                .find(|edge| {
                    !dispatch_loop_guard_can_emit(edge)
                        && !fallback_expression_guard_can_emit(edge)
                        && !decomposed_guard_can_emit(input, case.key, case.dispatch_index, edge)
                })
                .map(|edge| edge.guard_lowering)
        })
}

fn dispatch_loop_guard_can_emit(edge: &RuntimeDispatchLoopEdge) -> bool {
    match edge.guard_lowering {
        StateGuardLowering::NoOp => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    StateGuardOperator::Equal
                        | StateGuardOperator::NotEqual
                        | StateGuardOperator::Greater
                        | StateGuardOperator::GreaterOrEqual
                        | StateGuardOperator::Less
                        | StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::CompareRuntimeValue => {
            edge.guard_has_storage
                && edge.guard_has_right_storage
                && matches!(
                    edge.guard_operator,
                    StateGuardOperator::Equal
                        | StateGuardOperator::NotEqual
                        | StateGuardOperator::Greater
                        | StateGuardOperator::GreaterOrEqual
                        | StateGuardOperator::Less
                        | StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::NeedsRuntimeExpression => false,
    }
}

fn decomposed_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    let Some(clauses) = lower_guard_conjunction(
        input.state_guards,
        input.layouts,
        input.runtime_storage,
        input.entry_key.machine,
        source_key,
        source_key.machine,
        source_dispatch_index,
        edge.order,
    ) else {
        return false;
    };

    !clauses.is_empty()
        && clauses.iter().all(|clause| match clause.lowering {
            StateGuardLowering::CompareStaticValue => {
                clause.has_storage
                    && matches!(
                        clause.operator,
                        StateGuardOperator::Equal
                            | StateGuardOperator::NotEqual
                            | StateGuardOperator::Greater
                            | StateGuardOperator::GreaterOrEqual
                            | StateGuardOperator::Less
                            | StateGuardOperator::LessOrEqual
                    )
                    && matches!(clause.byte_size, 1 | 4 | 8)
            }
            StateGuardLowering::CompareRuntimeValue => {
                clause.has_storage
                    && clause.has_right_storage
                    && matches!(
                        clause.operator,
                        StateGuardOperator::Equal
                            | StateGuardOperator::NotEqual
                            | StateGuardOperator::Greater
                            | StateGuardOperator::GreaterOrEqual
                            | StateGuardOperator::Less
                            | StateGuardOperator::LessOrEqual
                    )
                    && matches!(clause.byte_size, 1 | 4 | 8)
            }
            _ => false,
        })
}

fn fallback_expression_guard_can_emit(edge: &RuntimeDispatchLoopEdge) -> bool {
    matches!(
        edge.guard_lowering,
        StateGuardLowering::CompareStaticValue
            | StateGuardLowering::CompareRuntimeValue
            | StateGuardLowering::NeedsRuntimeExpression
    ) && guard_expression_can_emit(&edge.guard)
}

fn guard_expression_can_emit(guard: &TransitionGuard) -> bool {
    let TransitionGuard::When(Expression::Binary(binary)) = guard else {
        return false;
    };

    match binary.operator {
        BinaryOperator::Equal
        | BinaryOperator::NotEqual
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual => {
            runtime_value_expression_can_emit(&binary.left)
                && runtime_value_expression_can_emit(&binary.right)
        }
        _ => false,
    }
}

fn runtime_value_expression_can_emit(expression: &Expression) -> bool {
    match expression {
        Expression::Binary(binary) => matches!(
            binary.operator,
            BinaryOperator::Add | BinaryOperator::Multiply | BinaryOperator::Subtract
        ) && runtime_value_expression_can_emit(&binary.left)
            && runtime_value_expression_can_emit(&binary.right),
        Expression::Name(_)
        | Expression::Member(_)
        | Expression::Indexed(_)
        | Expression::Mutable(_)
        | Expression::Boolean(_)
        | Expression::Integer(_) => true,
        _ => false,
    }
}
