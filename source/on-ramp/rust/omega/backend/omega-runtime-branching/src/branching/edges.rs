use crate::RuntimeBranchingContext;
use omega_control_flow::{MachineFlow, PlannedTransitionTarget, StateKey};
use omega_state_calls::StateCallRole;
use omega_state_graph::RuntimeTransitionTarget;
use omega_state_guards::classify_transition_guard_expression;
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

use super::lookups::state_statement_has_host_call;
use super::{RuntimeBranchTargetLowering, RuntimeBranchingCallEdge};

pub(super) fn build_branch_edges(
    context: &RuntimeBranchingContext,
    state_key: StateKey,
    expressions: &mut ExpressionTable,
    target_arguments: &mut Arena<ExpressionHandle>,
    target_values: &mut Arena<ExpressionHandle>,
    output_edges: &mut Arena<RuntimeBranchingCallEdge>,
) -> HandleSpan<RuntimeBranchingCallEdge> {
    let Some(machine) = context.control_flow.machine_by_symbol(state_key.machine) else {
        return HandleSpan::empty();
    };
    let Some(state) = context.control_flow.state_by_key(state_key) else {
        return HandleSpan::empty();
    };
    let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
        return HandleSpan::empty();
    };
    let state_dispatch_index = context
        .state_dispatch
        .states
        .iter()
        .find(|(_, dispatch_state)| dispatch_state.key == state_key)
        .map(|(_, dispatch_state)| dispatch_state.dispatch_index);

    output_edges.insert_many(transitions.iter().enumerate().map(|(order, transition)| {
        let guard = context
            .state_guards
            .guard_for_dispatch(state_dispatch_index.unwrap_or_default(), order)
            .and_then(|guard| guard.has_expression.then_some(guard.expression))
            .map(|guard| expressions.copy_from(&context.state_guards.expressions, guard))
            .unwrap_or_else(|| {
                transition
                    .expressions
                    .guard
                    .is_valid()
                    .then(|| {
                        expressions.copy_from(
                            &context.control_flow.expressions,
                            transition.expressions.guard,
                        )
                    })
                    .unwrap_or_else(ExpressionHandle::invalid)
            });
        let target = runtime_transition_target(context, machine, state.key, &transition.target);
        RuntimeBranchingCallEdge {
            order,
            statement_index: transition.statement_index,
            lowering: branch_target_lowering(context, &target),
            target,
            continuation: runtime_transition_target(
                context,
                machine,
                state.key,
                &transition.continuation,
            ),
            target_arguments: transition_target_arguments(
                context,
                transition.expressions.target_arguments,
                expressions,
                target_arguments,
            ),
            target_value: if transition.expressions.target_value.is_valid() {
                let copied = expressions.copy_from(
                    &context.control_flow.expressions,
                    transition.expressions.target_value,
                );
                target_values.append(copied);
                copied
            } else {
                ExpressionHandle::invalid()
            },
            guard_kind: classify_transition_guard_expression(expressions, guard),
            guard,
        }
    }))
}

fn transition_target_arguments(
    context: &RuntimeBranchingContext,
    arguments: HandleSpan<ExpressionHandle>,
    expressions: &mut ExpressionTable,
    arena: &mut Arena<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    arena.insert_many(
        context
            .control_flow
            .expressions
            .expression_handles(arguments)
            .iter()
            .map(|argument| expressions.copy_from(&context.control_flow.expressions, *argument)),
    )
}

fn branch_target_lowering(
    context: &RuntimeBranchingContext,
    target: &RuntimeTransitionTarget,
) -> RuntimeBranchTargetLowering {
    let RuntimeTransitionTarget::State { key, .. } = target else {
        return match target {
            RuntimeTransitionTarget::Terminal | RuntimeTransitionTarget::None => {
                RuntimeBranchTargetLowering::Terminal
            }
            RuntimeTransitionTarget::Unknown => RuntimeBranchTargetLowering::Unknown,
            RuntimeTransitionTarget::State { .. } => unreachable!(),
        };
    };

    let Some(target_state) = context.control_flow.state_by_key(*key) else {
        return RuntimeBranchTargetLowering::Unknown;
    };

    if context
        .control_flow
        .transitions
        .span(target_state.transitions)
        .is_some_and(|transitions| !transitions.is_empty())
    {
        return RuntimeBranchTargetLowering::InlineBranching;
    }

    // A target that performs ANY state call -- including `let x = self.f()`
    // (AssignmentValue) or `g(self.f())` (CallArgument), not just a bare statement
    // call -- must lower as a straight-line branch: the straight-line expansion emits
    // and expands nested StateCall operations, whereas the leaf expansion drops them
    // (RuntimeLeafBranchOperationKind has no StateCall variant). Misclassifying such a
    // target as a leaf silently dropped the call, leaving its `&mut`/value result
    // uninitialized (null) -> the next use faulted.
    let has_state_call = context.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.source_key == *key
            && matches!(
                state_call.role,
                StateCallRole::Statement
                    | StateCallRole::AssignmentValue
                    | StateCallRole::CallArgument
            )
            && !state_statement_has_host_call(context, *key, state_call.statement_index)
    });

    if has_state_call {
        RuntimeBranchTargetLowering::InlineStraightLine
    } else {
        RuntimeBranchTargetLowering::InlineLeaf
    }
}

fn runtime_transition_target(
    context: &RuntimeBranchingContext,
    machine: &MachineFlow,
    current_state: StateKey,
    target: &PlannedTransitionTarget,
) -> RuntimeTransitionTarget {
    match target {
        PlannedTransitionTarget::None => RuntimeTransitionTarget::None,
        PlannedTransitionTarget::State { key, .. } => RuntimeTransitionTarget::root_state(*key),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            ..
        } => {
            if *receiver_symbol == current_state.machine || receiver.as_str() == "self" {
                if let Some(key) = context
                    .control_flow
                    .state_key_by_symbols(current_state.machine, *state_symbol)
                {
                    return RuntimeTransitionTarget::root_state(key);
                }

                return resolve_attached_machine_state_key(context, machine, *state_symbol)
                    .map(RuntimeTransitionTarget::root_state)
                    .unwrap_or(RuntimeTransitionTarget::Unknown);
            }

            context
                .control_flow
                .machine_contains(machine)
                .iter()
                .find(|contained| {
                    receiver_symbol.is_valid() && contained.symbol == *receiver_symbol
                })
                .and_then(|contained| {
                    resolve_attached_data_state_key(context, &contained.type_name, *state_symbol)
                })
                .map(RuntimeTransitionTarget::root_state)
                .unwrap_or(RuntimeTransitionTarget::Unknown)
        }
        PlannedTransitionTarget::SelfTarget => RuntimeTransitionTarget::root_state(current_state),
        PlannedTransitionTarget::Terminal => RuntimeTransitionTarget::Terminal,
    }
}

fn resolve_attached_machine_state_key(
    context: &RuntimeBranchingContext,
    source_machine: &MachineFlow,
    target_symbol: psi_symbols::SymbolHandle,
) -> Option<StateKey> {
    let attached_data = source_machine.attached_data.as_ref()?;
    resolve_attached_data_state_key(context, attached_data, target_symbol)
}

fn resolve_attached_data_state_key(
    context: &RuntimeBranchingContext,
    attached_data: &psi_checked_trees::name::Identifier,
    target_symbol: psi_symbols::SymbolHandle,
) -> Option<StateKey> {
    if !target_symbol.is_valid() {
        return None;
    }

    context
        .control_flow
        .machines
        .iter()
        .filter_map(|(_, candidate)| {
            (candidate.attached_data.as_ref() == Some(attached_data)).then_some(candidate)
        })
        .find_map(|candidate| {
            context
                .control_flow
                .states
                .span(candidate.states)?
                .iter()
                .find(|state| state.key.state == target_symbol && state.key.segment_index == 0)
                .map(|state| state.key)
        })
}
