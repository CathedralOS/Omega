use crate::control_flow::{MachineFlow, OperationKind, PlannedTransitionTarget, StateKey};
use crate::plan::NativePlan;
use crate::runtime_dispatch::guards::classify_transition_guard;
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;

use super::lookups::state_statement_has_host_call;
use super::{RuntimeBranchTargetLowering, RuntimeBranchingCallEdge};

pub(super) fn build_branch_edges(
    native_plan: &NativePlan,
    state_key: StateKey,
    target_arguments: &mut Arena<Expression>,
) -> Vec<RuntimeBranchingCallEdge> {
    let Some(machine) = native_plan
        .control_flow
        .machine_by_symbol(state_key.machine)
    else {
        return Vec::new();
    };
    let Some(state) = native_plan.control_flow.state_by_key(state_key) else {
        return Vec::new();
    };
    let Some(transitions) = native_plan.control_flow.transitions.span(state.transitions) else {
        return Vec::new();
    };

    transitions
        .iter()
        .enumerate()
        .map(|(order, transition)| {
            let target =
                runtime_transition_target(native_plan, machine, state.key, &transition.target);
            RuntimeBranchingCallEdge {
                order,
                lowering: branch_target_lowering(native_plan, &target),
                target,
                continuation: transition
                    .continuation
                    .as_ref()
                    .map(|continuation| {
                        runtime_transition_target(native_plan, machine, state.key, continuation)
                    })
                    .unwrap_or(RuntimeTransitionTarget::None),
                target_arguments: transition_target_arguments(&transition.target, target_arguments),
                guard_kind: classify_transition_guard(&transition.guard),
                guard: transition.guard.clone(),
            }
        })
        .collect()
}

fn transition_target_arguments(
    target: &PlannedTransitionTarget,
    arena: &mut Arena<Expression>,
) -> HandleSpan<Expression> {
    match target {
        PlannedTransitionTarget::State { arguments, .. }
        | PlannedTransitionTarget::Nested { arguments, .. } => arena.insert_many(arguments.clone()),
        PlannedTransitionTarget::SelfTarget | PlannedTransitionTarget::Terminal => {
            HandleSpan::empty()
        }
    }
}

fn branch_target_lowering(
    native_plan: &NativePlan,
    target: &RuntimeTransitionTarget,
) -> RuntimeBranchTargetLowering {
    let RuntimeTransitionTarget::State { key, .. } = target else {
        return match target {
            RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
                RuntimeBranchTargetLowering::Terminal
            }
            RuntimeTransitionTarget::Unknown { .. } => RuntimeBranchTargetLowering::Unknown,
            RuntimeTransitionTarget::State { .. } => unreachable!(),
        };
    };

    let Some(target_state) = native_plan.control_flow.state_by_key(*key) else {
        return RuntimeBranchTargetLowering::Unknown;
    };

    if native_plan
        .control_flow
        .transitions
        .span(target_state.transitions)
        .is_some_and(|transitions| !transitions.is_empty())
    {
        return RuntimeBranchTargetLowering::InlineBranching;
    }

    let has_state_call = native_plan
        .control_flow
        .operations
        .span(target_state.operations)
        .is_some_and(|operations| {
            operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::Call { .. })
                    && !state_statement_has_host_call(native_plan, *key, operation.statement_index)
            })
        });

    if has_state_call {
        RuntimeBranchTargetLowering::InlineStraightLine
    } else {
        RuntimeBranchTargetLowering::InlineLeaf
    }
}

fn runtime_transition_target(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    current_state: StateKey,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::State { key, .. } => RuntimeTransitionTarget::State { key: *key },
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state,
            ..
        } => machine
            .contains
            .iter()
            .find(|contained| receiver_symbol.is_valid() && contained.symbol == *receiver_symbol)
            .and_then(|contained| {
                native_plan
                    .control_flow
                    .machine_by_symbol(contained.type_symbol)
            })
            .and_then(|target_machine| {
                native_plan
                    .control_flow
                    .states
                    .span(target_machine.states)
                    .and_then(|states| {
                        states.iter().find(|candidate| {
                            state_symbol.is_valid() && candidate.key.state == *state_symbol
                        })
                    })
            })
            .map(|target_state| RuntimeTransitionTarget::State {
                key: target_state.key,
            })
            .unwrap_or_else(|| RuntimeTransitionTarget::Unknown {
                name: format!("{receiver}.{state}"),
            }),
        PlannedTransitionTarget::SelfTarget => {
            RuntimeTransitionTarget::State { key: current_state }
        }
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}
