use crate::EmissionPlanningInput;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardLowering, StateGuardOperator, lower_guard_conjunction};
use omega_state_schedule::{ScheduledState, ScheduledStateCollector};
use psi_arena::Arena;
use psi_checked_trees::statement::TransitionGuard;

use super::{
    EmissionBlocker, blocker,
    guard_expression_support::transition_guard_can_emit,
    semantic_scope::{proof_scope_suffix, state_name},
};

pub(super) fn runtime_and_required_states(
    input: &EmissionPlanningInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = ScheduledStateCollector::new();

    for (_, state) in input.runtime_flow.states.iter() {
        states.push_key(state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if state_call.required {
            states.push_key(state_call.source_key);

            if state_call.target_key.is_valid() {
                states.push_key(state_call.target_key);
            }
        }
    }

    states.finish()
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
    if let Some((source_key, guard_lowering)) = first_unsupported_dispatch_guard(input) {
        return blocker(
            "runtime dispatch",
            &format!(
                "transition guard in state {3} could not be lowered to a native comparison: a \
                 guard subject must be a conjunction (`&&`) of simple-value comparisons, \
                 optionally within a top-level disjunction (`||`). (Shifts and casts in a guard \
                 subject DO lower directly.) Bind a still-unsupported subexpression -- a \
                 value-call result, or a disjunction (`||`) nested inside a conjunction (`&&`, \
                 e.g. `a && (b || c)`) -- to a field or local first, then compare that local. \
                 (guard lowering {guard_lowering:?}; dispatch loop {0} case(s), {1} edge(s), {2} \
                 cycle(s)){4}",
                input.runtime_dispatch_loop.cases.len(),
                input.runtime_dispatch_loop.edges.len(),
                input.runtime_flow.cycles.len(),
                state_name(input, source_key),
                proof_scope_suffix(input, source_key)
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
                    || fallback_expression_guard_can_emit(
                        input,
                        case.key,
                        case.dispatch_index,
                        edge,
                    )
                    || decomposed_guard_can_emit(input, case.key, case.dispatch_index, edge))
                    && edge.action != RuntimeDispatchLoopAction::Unknown
            })
    })
}

fn first_unsupported_dispatch_guard(
    input: &EmissionPlanningInput<'_>,
) -> Option<(omega_control_flow::StateKey, StateGuardLowering)> {
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
                        && !fallback_expression_guard_can_emit(
                            input,
                            case.key,
                            case.dispatch_index,
                            edge,
                        )
                        && !decomposed_guard_can_emit(input, case.key, case.dispatch_index, edge)
                })
                .map(|edge| (case.key, edge.guard_lowering))
        })
}

fn dispatch_loop_guard_can_emit(edge: &RuntimeDispatchLoopEdge) -> bool {
    match edge.guard_lowering {
        // ForwardBranchSkip / BranchArmsEnd are only ever produced for inline leaf
        // arms, never as a dispatch-edge guard, so they are trivially emittable here.
        StateGuardLowering::NoOp
        | StateGuardLowering::ForwardBranchSkip
        | StateGuardLowering::BranchArmsEnd => true,
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
        // The leaf-arm poison markers never appear on a dispatch edge.
        StateGuardLowering::NeedsRuntimeExpression
        | StateGuardLowering::UnresolvedInlineArmGuard
        | StateGuardLowering::UnloweredTerminalHostCall
        | StateGuardLowering::UnloweredCaseLiteralField => false,
    }
}

fn decomposed_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    let clauses = lower_guard_conjunction(
        input.state_guards,
        input.layouts,
        input.runtime_storage,
        input.receiver_bases,
        input.state_contexts,
        input.entry_key.machine,
        source_key,
        source_key.machine,
        source_dispatch_index,
        edge.order,
    );

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

fn fallback_expression_guard_can_emit(
    input: &EmissionPlanningInput<'_>,
    source_key: omega_control_flow::StateKey,
    source_dispatch_index: u32,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    matches!(
        edge.guard_lowering,
        StateGuardLowering::CompareStaticValue
            | StateGuardLowering::CompareRuntimeValue
            | StateGuardLowering::NeedsRuntimeExpression
    ) && {
        let guard = transition_guard_for_edge(input, edge);
        transition_guard_can_emit(
            input,
            source_key,
            source_dispatch_index,
            edge.statement_index,
            &guard,
        )
    }
}

fn transition_guard_for_edge(
    input: &EmissionPlanningInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
) -> TransitionGuard {
    if edge.guard_has_expression {
        TransitionGuard::When(
            input
                .state_guards
                .expressions
                .to_tree(edge.guard_expression),
        )
    } else {
        TransitionGuard::Always
    }
}
