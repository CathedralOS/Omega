use crate::StateCallPlanningContext;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::name::ProgramName;
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, OperationExpressionRefs, OperationKind, StateKey,
    TransitionExpressionRefs,
};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

use super::{StateCallResolution, StateCallRole};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectedStateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
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
            let mut call_ordinal = 0usize;
            if let OperationKind::Call {
                receiver_symbol,
                target_symbol,
                receiver,
                target,
            } = &operation.kind
            {
                if context
                    .state_statement_has_host_call_by_key(state.key, operation.statement_index)
                {
                    continue;
                }

                let resolved_target = resolve_state_call_target(
                    &context.control_flow,
                    machine,
                    state.key,
                    *receiver_symbol,
                    *target_symbol,
                    receiver.as_ref().map(|receiver| receiver.members()),
                    target,
                );

                calls.push(CollectedStateCall {
                    source_key: state.key,
                    statement_index: operation.statement_index,
                    call_ordinal,
                    role: StateCallRole::Statement,
                    receiver: receiver
                        .as_ref()
                        .and_then(|receiver: &omega_checked_trees::expression::NamePath| {
                            receiver.last().cloned()
                        })
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
                call_ordinal += 1;
            }

            collect_expression_state_calls_for_operation(
                context,
                machine,
                state.key,
                operation.statement_index,
                &mut call_ordinal,
                operation.expressions,
                &mut calls,
            );
        }

        let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
            continue;
        };

        for (statement_index, transition) in transitions.iter().enumerate() {
            let mut call_ordinal = 0usize;
            collect_expression_state_calls_for_transition(
                context,
                machine,
                state.key,
                statement_index,
                &mut call_ordinal,
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
    call_ordinal: &mut usize,
    expressions: OperationExpressionRefs,
    calls: &mut Vec<CollectedStateCall>,
) {
    match expressions {
        OperationExpressionRefs::Assignment { value, .. } => collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            StateCallRole::AssignmentValue,
            value,
            calls,
        ),
        OperationExpressionRefs::Call { arguments } => {
            for argument in context
                .control_flow
                .expressions
                .expression_handles(arguments)
            {
                collect_expression_state_calls(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    call_ordinal,
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
            call_ordinal,
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
    call_ordinal: &mut usize,
    expressions: TransitionExpressionRefs,
    calls: &mut Vec<CollectedStateCall>,
) {
    if let Some(guard) = expressions.guard {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
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
            call_ordinal,
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
            call_ordinal,
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
            call_ordinal,
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
            call_ordinal,
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
    call_ordinal: &mut usize,
    role: StateCallRole,
    expression: ExpressionHandle,
    calls: &mut Vec<CollectedStateCall>,
) {
    collect_expression_state_calls_in_table(
        context,
        machine,
        source_key,
        statement_index,
        call_ordinal,
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
    call_ordinal: &mut usize,
    role: StateCallRole,
    expression: ExpressionHandle,
    calls: &mut Vec<CollectedStateCall>,
) {
    match context
        .control_flow
        .expressions
        .expression(expression)
        .clone()
    {
        ExpressionNode::ArrayLiteral(values) => {
            for value in context.control_flow.expressions.expression_handles(values) {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    call_ordinal,
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
                call_ordinal,
                role,
                binary.left,
                calls,
            );
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                call_ordinal,
                role,
                binary.right,
                calls,
            );
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) =
                call_receiver_parts(&context.control_flow.expressions, call.receiver);
            let receiver_members = receiver_path.as_ref().map(ReceiverPath::members);
            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                source_key,
                receiver_symbol,
                call.target_symbol,
                receiver_members,
                &call.target,
            );
            let is_machine_call = resolved_target.is_some()
                || receiver_can_dispatch_to_machine(
                    &context.control_flow,
                    machine,
                    source_key,
                    receiver_symbol,
                    receiver_members,
                );
            if !is_machine_call {
                if call.receiver.is_valid() {
                    collect_expression_state_calls_in_table(
                        context,
                        machine,
                        source_key,
                        statement_index,
                        call_ordinal,
                        role,
                        call.receiver,
                        calls,
                    );
                }
                for argument in context
                    .control_flow
                    .expressions
                    .expression_handles(call.arguments)
                {
                    collect_expression_state_calls_in_table(
                        context,
                        machine,
                        source_key,
                        statement_index,
                        call_ordinal,
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
                call_ordinal: *call_ordinal,
                role,
                receiver: receiver_path
                    .as_ref()
                    .and_then(|receiver| receiver.members().last().cloned())
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
            *call_ordinal += 1;

            if call.receiver.is_valid() {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    call_ordinal,
                    role,
                    call.receiver,
                    calls,
                );
            }
            for argument in context
                .control_flow
                .expressions
                .expression_handles(call.arguments)
            {
                collect_expression_state_calls_in_table(
                    context,
                    machine,
                    source_key,
                    statement_index,
                    call_ordinal,
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
            call_ordinal,
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
                call_ordinal,
                role,
                indexed.collection,
                calls,
            );
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                call_ordinal,
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
            call_ordinal,
            role,
            member.receiver,
            calls,
        ),
        ExpressionNode::Mutable(inner) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
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
                    call_ordinal,
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
) -> (SymbolHandle, Option<ReceiverPath<'_>>) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match expressions.expression(receiver) {
        ExpressionNode::Mutable(inner) => call_receiver_parts(expressions, *inner),
        ExpressionNode::Name(path) => (
            path.symbol,
            Some(ReceiverPath::borrowed(
                expressions.name_path_members(path.members),
            )),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(expressions, member.receiver);
            let mut members = path.map(ReceiverPath::into_owned).unwrap_or_default();
            members.push(member.member.clone());
            (member.member_symbol, Some(ReceiverPath::owned(members)))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

enum ReceiverPath<'table> {
    Borrowed(&'table [ProgramName]),
    Owned(Vec<ProgramName>),
}

impl<'table> ReceiverPath<'table> {
    fn borrowed(members: &'table [ProgramName]) -> Self {
        Self::Borrowed(members)
    }

    fn owned(members: Vec<ProgramName>) -> Self {
        Self::Owned(members)
    }

    fn members(&self) -> &[ProgramName] {
        match self {
            Self::Borrowed(members) => members,
            Self::Owned(members) => members,
        }
    }

    fn into_owned(self) -> Vec<ProgramName> {
        match self {
            Self::Borrowed(members) => members.to_vec(),
            Self::Owned(members) => members,
        }
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
    if receiver.is_none()
        || receiver_symbol == machine.symbol
        || receiver
            .is_some_and(|receiver| matches!(receiver, [member] if member.as_str() == "self"))
    {
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

    let receiver_name = receiver.and_then(|receiver| receiver.last());
    if let Some(contained) = control_flow
        .machine_contains(machine)
        .iter()
        .find(|contained| {
            (receiver_symbol.is_valid() && contained.symbol == receiver_symbol)
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

    if receiver_symbol.is_valid() {
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
    if state_symbol.is_valid()
        && let Some(key) = control_flow.state_key_by_symbols(machine_symbol, state_symbol)
    {
        return Some(key);
    }

    let machine = control_flow.machine_by_symbol(machine_symbol)?;
    control_flow
        .states
        .span(machine.states)?
        .iter()
        .find(|state| state.key.machine == machine_symbol && state.name == *state_name)
        .map(|state| state.key)
}

fn receiver_can_dispatch_to_machine(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
) -> bool {
    if receiver.is_none()
        || receiver_symbol == machine.symbol
        || receiver
            .is_some_and(|receiver| matches!(receiver, [member] if member.as_str() == "self"))
    {
        return false;
    }

    if !receiver_symbol.is_valid() {
        let receiver_name = receiver.and_then(|receiver| receiver.last());
        return control_flow
            .machine_contains(machine)
            .iter()
            .any(|contained| {
                receiver_name.is_some_and(|receiver_name| contained.name == *receiver_name)
            });
    }

    let receiver_name = receiver.and_then(|receiver| receiver.last());
    if control_flow
        .machine_contains(machine)
        .iter()
        .any(|contained| {
            contained.symbol == receiver_symbol
                || receiver_name.is_some_and(|receiver_name| contained.name == *receiver_name)
        })
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
    let state = control_flow
        .states
        .iter()
        .find_map(|(_, state)| (state.key == source_key).then_some(state))?;
    control_flow
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| parameter.type_symbol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StateCallPlanningContext;
    use omega_calling_conventions::build_host_abi_plan;
    use omega_platform_interface::build_host_call_plan;
    use omega_source_files_to_tokens::Lexer;
    use omega_state_graph::build_runtime_flow_plan;
    use omega_state_graph_to_control_flow::build_control_flow_plan;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;
    use omega_typed_trees_to_checked_trees::lower_typed_trees;

    #[test]
    fn collects_contained_assignment_value_call() {
        let source = r#"
            data Reward { gold: i32; }
            data Random {}
            data main { rng: Random; }

            machine Random::one -> i32 {
                pub entry(&mut self) {
                    -> 1;
                }
            }

            machine main {
                pub entry(&mut self, reward: &mut Reward) {
                    reward.gold = 1 + self.rng.one();
                }
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(&typed).expect("check");
        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let entry_machine_symbol = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "main")
            .map(|(_, machine)| machine.symbol)
            .expect("main machine");
        let entry_key = control_flow
            .states
            .iter()
            .find(|(_, state)| {
                state.key.machine == entry_machine_symbol && state.name.as_str() == "entry"
            })
            .map(|(_, state)| state.key)
            .expect("entry state");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let context = StateCallPlanningContext {
            control_flow: control_flow.clone(),
            host_calls,
            runtime_flow,
        };
        let machine = control_flow
            .machine_by_symbol(entry_machine_symbol)
            .expect("machine flow");

        let calls = collect_machine_state_calls(&context, machine);
        assert!(
            calls.iter().any(|call| {
                call.role == StateCallRole::AssignmentValue
                    && call.source_key.machine == entry_key.machine
                    && call.source_key.state == entry_key.state
                    && call.statement_index == 0
                    && call.receiver.as_str() == "rng"
                    && call.resolution == StateCallResolution::ContainedMachine
            }),
            "expected contained assignment-value call, got {calls:?}"
        );
    }
}
