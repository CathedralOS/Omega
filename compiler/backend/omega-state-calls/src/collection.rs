use crate::StateCallPlanningContext;
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, OperationExpressionRefs, OperationKind, StateKey,
    TransitionExpressionRefs,
};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_typed_trees::name::ProgramName;

use super::{StateCallResolution, StateCallRole};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectedStateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub role: StateCallRole,
    pub receiver: ProgramName,
    pub target_key: StateKey,
    pub raw_arguments: HandleSpan<ExpressionHandle>,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
}

pub(crate) fn collect_machine_state_calls(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
) -> Vec<CollectedStateCall> {
    let mut calls = Vec::new();

    let Some(states) = context.control_flow.states.span(machine.states) else {
        return calls;
    };

    for state in states {
        let Some(operations) = context.control_flow.operations.span(state.operations) else {
            continue;
        };

        for operation in operations {
            let OperationKind::Call {
                receiver_symbol,
                target_symbol,
                receiver,
                target,
            } = &operation.kind
            else {
                continue;
            };

            if context.state_statement_has_host_call_by_key(state.key, operation.statement_index) {
                continue;
            }

            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                state.key,
                *receiver_symbol,
                *target_symbol,
                receiver.as_ref().map(|receiver| receiver.as_slice()),
                target,
            );

            calls.push(CollectedStateCall {
                source_key: state.key,
                statement_index: operation.statement_index,
                role: StateCallRole::Statement,
                receiver: receiver
                    .as_ref()
                    .and_then(|receiver| receiver.as_slice().last().cloned())
                    .unwrap_or_else(|| ProgramName::generated("self")),
                target_key: resolved_target
                    .as_ref()
                    .map(|target| target.key)
                    .unwrap_or_default(),
                raw_arguments: match operation.expressions {
                    OperationExpressionRefs::Call { arguments } => arguments,
                    _ => HandleSpan::empty(),
                },
                reachable: context.runtime_state_is_reachable_by_key(state.key),
                required: false,
                resolution: resolved_target
                    .map(|target| target.resolution)
                    .unwrap_or(StateCallResolution::Unresolved),
            });

            collect_expression_state_calls_for_operation(
                context,
                machine,
                state.key,
                operation.statement_index,
                operation.expressions,
                &mut calls,
            );
        }

        let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
            continue;
        };

        for (statement_index, transition) in transitions.iter().enumerate() {
            collect_expression_state_calls_for_transition(
                context,
                machine,
                state.key,
                statement_index,
                transition.expressions,
                &mut calls,
            );
        }
    }

    calls
}

fn collect_expression_state_calls_for_operation(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
    source_key: StateKey,
    statement_index: usize,
    expressions: OperationExpressionRefs,
    calls: &mut Vec<CollectedStateCall>,
) {
    match expressions {
        OperationExpressionRefs::Assignment { value, .. } => collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::AssignmentValue,
            value,
            calls,
        ),
        OperationExpressionRefs::Call { arguments } => {
            for argument in context.control_flow.expressions.expression_handles(arguments) {
                collect_expression_state_calls(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    StateCallRole::CallArgument,
                    *argument,
                    calls,
                );
            }
        }
        OperationExpressionRefs::Expression(expression) => collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::AssignmentValue,
            expression,
            calls,
        ),
        OperationExpressionRefs::None => {}
    }
}

fn collect_expression_state_calls_for_transition(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
    source_key: StateKey,
    statement_index: usize,
    expressions: TransitionExpressionRefs,
    calls: &mut Vec<CollectedStateCall>,
) {
    if let Some(guard) = expressions.guard {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::TransitionGuard,
            guard,
            calls,
        );
    }

    for argument in context
        .control_flow
        .expressions
        .expression_handles(expressions.target_arguments)
    {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::TransitionArgument,
            *argument,
            calls,
        );
    }

    if let Some(value) = expressions.target_value {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::TransitionArgument,
            value,
            calls,
        );
    }

    for argument in context
        .control_flow
        .expressions
        .expression_handles(expressions.continuation_arguments)
    {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::TransitionArgument,
            *argument,
            calls,
        );
    }

    if let Some(value) = expressions.continuation_value {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            StateCallRole::TransitionArgument,
            value,
            calls,
        );
    }
}

fn collect_expression_state_calls(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    expression: ExpressionHandle,
    calls: &mut Vec<CollectedStateCall>,
) {
    collect_expression_state_calls_in_table(
        context,
        machine,
        source_key,
        statement_index,
        role,
        expression,
        calls,
    );
}

fn collect_expression_state_calls_in_table(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    expression: ExpressionHandle,
    calls: &mut Vec<CollectedStateCall>,
) {
    match context.control_flow.expressions.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            for value in context.control_flow.expressions.expression_handles(values) {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    role,
                    *value,
                    calls,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                role,
                binary.left,
                calls,
            );
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                role,
                binary.right,
                calls,
            );
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(&context.control_flow.expressions, call.receiver);
            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                source_key,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            );
            let is_machine_call = resolved_target.is_some()
                || receiver_can_dispatch_to_machine(
                    &context.control_flow,
                    machine,
                    source_key,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );
            if !is_machine_call {
                if call.receiver.is_valid() {
                    collect_expression_state_calls_in_table(
                        context,
                        machine,
                        source_key,
                        statement_index,
                        role,
                        call.receiver,
                        calls,
                    );
                }
                for argument in context.control_flow.expressions.expression_handles(call.arguments) {
                    collect_expression_state_calls_in_table(
                        context,
                        machine,
                        source_key,
                        statement_index,
                        role,
                        *argument,
                        calls,
                    );
                }
                return;
            }
            calls.push(CollectedStateCall {
                source_key,
                statement_index,
                role,
                receiver: receiver_path
                    .as_ref()
                    .and_then(|receiver| receiver.last().cloned())
                    .unwrap_or_else(|| ProgramName::generated("self")),
                target_key: resolved_target
                    .as_ref()
                    .map(|target| target.key)
                    .unwrap_or_default(),
                raw_arguments: call.arguments,
                reachable: context.runtime_state_is_reachable_by_key(source_key),
                required: false,
                resolution: resolved_target
                    .map(|target| target.resolution)
                    .unwrap_or(StateCallResolution::Unresolved),
            });

            if call.receiver.is_valid() {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    role,
                    call.receiver,
                    calls,
                );
            }
            for argument in context.control_flow.expressions.expression_handles(call.arguments) {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    role,
                    *argument,
                    calls,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            role,
            cast.value,
            calls,
        ),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                role,
                indexed.collection,
                calls,
            );
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                role,
                indexed.index,
                calls,
            );
        }
        ExpressionNode::Member(member) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            role,
            member.receiver,
            calls,
        ),
        ExpressionNode::Mutable(inner) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            role,
            inner,
            calls,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in context
                .control_flow
                .expressions
                .struct_fields(struct_literal.fields)
            {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    role,
                    field.value,
                    calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn call_receiver_parts(
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
) -> (SymbolHandle, Option<Vec<ProgramName>>) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match expressions.expression(receiver).clone() {
        ExpressionNode::Mutable(inner) => call_receiver_parts(expressions, inner),
        ExpressionNode::Name(path) => (
            path.symbol,
            Some(expressions.name_path_members(path.members).to_vec()),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(expressions, member.receiver);
            let mut path = path.unwrap_or_default();
            path.push(member.member.clone());
            (member.member_symbol, Some(path))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

struct ResolvedStateCall {
    key: StateKey,
    resolution: StateCallResolution,
}

fn resolve_state_call_target(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
    target_state: &ProgramName,
) -> Option<ResolvedStateCall> {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == ["self"]) {
        return resolve_state_key_in_machine(
            control_flow,
            machine.symbol,
            target_symbol,
            target_state,
        )
        .map(|key| ResolvedStateCall {
            key,
            resolution: StateCallResolution::Local,
        });
    }

    if receiver_symbol.is_valid() {
        let receiver_name = receiver.and_then(|receiver| receiver.last());

        if let Some(contained) = machine
            .contains
            .iter()
            .find(|contained| {
                contained.symbol == receiver_symbol
                    || receiver_name.is_some_and(|receiver_name| contained.name == *receiver_name)
            })
        {
            return resolve_state_key_in_machine(
                control_flow,
                contained.type_symbol,
                target_symbol,
                target_state,
            )
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::ContainedMachine,
            });
        }

        if let Some(target_machine) = control_flow.machine_by_symbol(receiver_symbol) {
            return resolve_state_key_in_machine(
                control_flow,
                target_machine.symbol,
                target_symbol,
                target_state,
            )
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::NamedMachine,
            });
        }

        if let Some(type_symbol) =
            source_state_parameter_machine_symbol(control_flow, source_key, receiver_symbol)
            && let Some(target_machine) = control_flow.machine_by_symbol(type_symbol)
        {
            return resolve_state_key_in_machine(
                control_flow,
                target_machine.symbol,
                target_symbol,
                target_state,
            )
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::NamedMachine,
            });
        }

        if target_symbol.is_valid()
            && let Some((key, _)) = control_flow
                .states
                .iter()
                .find(|(_, state)| state.key.state == target_symbol && state.key.segment_index == 0)
                .map(|(_, state)| (state.key, state.name.clone()))
        {
            return Some(ResolvedStateCall {
                key,
                resolution: StateCallResolution::ContainedMachine,
            });
        }

        return None;
    }

    let _ = receiver?;
    let _ = target_state;
    None
}

fn resolve_state_key_in_machine(
    control_flow: &ControlFlowPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    state_name: &ProgramName,
) -> Option<StateKey> {
    if state_symbol.is_valid() {
        control_flow.state_key_by_symbols(machine_symbol, state_symbol)
    } else {
        let _ = (control_flow, machine_symbol, state_name);
        None
    }
}

fn receiver_can_dispatch_to_machine(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
) -> bool {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == ["self"]) {
        return true;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    if machine
        .contains
        .iter()
        .any(|contained| contained.symbol == receiver_symbol)
    {
        return true;
    }

    control_flow.machine_by_symbol(receiver_symbol).is_some()
        || source_state_parameter_machine_symbol(control_flow, source_key, receiver_symbol)
            .and_then(|type_symbol| control_flow.machine_by_symbol(type_symbol))
            .is_some()
}

fn source_state_parameter_machine_symbol(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    let state = control_flow.states.iter().find_map(|(_, state)| {
        (state.key == source_key).then_some(state)
    })?;
    state
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| parameter.type_symbol)
}
