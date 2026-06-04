use crate::StateCallPlanningContext;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::name::Identifier;
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
    pub receiver_symbol: SymbolHandle,
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
    let mut calls = Vec::with_capacity(estimated_machine_state_call_capacity(context, machine));

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
                has_receiver,
                receiver: _,
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
                    *has_receiver,
                    target,
                );

                calls.push(CollectedStateCall {
                    source_key: state.key,
                    statement_index: operation.statement_index,
                    call_ordinal,
                    role: StateCallRole::Statement,
                    receiver_symbol: *receiver_symbol,
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

            if !context.state_statement_has_host_call_by_key(state.key, operation.statement_index) {
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
        }

        let Some(transitions) = context.control_flow.transitions.span(state.transitions) else {
            continue;
        };

        for transition in transitions {
            let mut call_ordinal = 0usize;
            collect_expression_state_calls_for_transition(
                context,
                machine,
                state.key,
                transition.statement_index,
                &mut call_ordinal,
                transition.expressions,
                &mut calls,
            );
        }
    }

    calls
}

fn estimated_machine_state_call_capacity(
    context: &StateCallPlanningContext,
    machine: &MachineFlow,
) -> usize {
    let Some(states) = context.control_flow.states.span(machine.states) else {
        return 0;
    };

    states
        .iter()
        .map(|state| {
            let operation_capacity = context
                .control_flow
                .operations
                .span(state.operations)
                .map(|operations| {
                    operations
                        .iter()
                        .map(|operation| {
                            usize::from(matches!(operation.kind, OperationKind::Call { .. }))
                                + operation_expression_call_capacity(operation.expressions)
                        })
                        .sum::<usize>()
                })
                .unwrap_or(0);

            let transition_capacity = context
                .control_flow
                .transitions
                .span(state.transitions)
                .map(|transitions| {
                    transitions
                        .iter()
                        .map(|transition| {
                            transition_expression_call_capacity(transition.expressions)
                        })
                        .sum::<usize>()
                })
                .unwrap_or(0);

            operation_capacity.saturating_add(transition_capacity)
        })
        .sum()
}

fn operation_expression_call_capacity(expressions: OperationExpressionRefs) -> usize {
    match expressions {
        OperationExpressionRefs::Assignment { value, .. }
        | OperationExpressionRefs::Expression(value) => usize::from(value.is_valid()),
        OperationExpressionRefs::Call { arguments } => arguments.len(),
        OperationExpressionRefs::None => 0,
    }
}

fn transition_expression_call_capacity(expressions: TransitionExpressionRefs) -> usize {
    usize::from(expressions.guard.is_valid())
        .saturating_add(expressions.target_arguments.len())
        .saturating_add(usize::from(expressions.target_value.is_valid()))
        .saturating_add(expressions.continuation_arguments.len())
        .saturating_add(usize::from(expressions.continuation_value.is_valid()))
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
    if expressions.guard.is_valid() {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            StateCallRole::TransitionGuard,
            expressions.guard,
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

    if expressions.target_value.is_valid() {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            StateCallRole::TransitionArgument,
            expressions.target_value,
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

    if expressions.continuation_value.is_valid() {
        collect_expression_state_calls(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            StateCallRole::TransitionArgument,
            expressions.continuation_value,
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
    match context.control_flow.expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in context.control_flow.expressions.expression_handles(*values) {
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
            let receiver = call_receiver_parts(&context.control_flow.expressions, call.receiver);
            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                source_key,
                receiver.symbol,
                call.target_symbol,
                receiver.is_present,
                &call.target,
            );
            let is_machine_call = resolved_target.is_some()
                || receiver_can_dispatch_to_machine(
                    &context.control_flow,
                    machine,
                    source_key,
                    receiver.symbol,
                    receiver.is_present,
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
                receiver_symbol: receiver.symbol,
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
        ExpressionNode::Range(range) => {
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                call_ordinal,
                role,
                range.start,
                calls,
            );
            collect_expression_state_calls_in_table(
                context,
                machine,
                source_key,
                statement_index,
                call_ordinal,
                role,
                range.end,
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
            *inner,
            calls,
        ),
        ExpressionNode::Unary(unary) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            role,
            unary.operand,
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

fn call_receiver_parts(expressions: &ExpressionTable, receiver: ExpressionHandle) -> ReceiverParts {
    if !receiver.is_valid() {
        return ReceiverParts::default();
    }

    match expressions.expression(receiver) {
        ExpressionNode::Mutable(inner) => call_receiver_parts(expressions, *inner),
        ExpressionNode::Name(path) => ReceiverParts {
            symbol: path.symbol,
            is_present: true,
        },
        ExpressionNode::Member(member) => {
            let mut receiver = call_receiver_parts(expressions, member.receiver);
            receiver.symbol = member.member_symbol;
            receiver.is_present = true;
            receiver
        }
        _ => ReceiverParts::default(),
    }
}

#[derive(Debug, Clone, Default)]
struct ReceiverParts {
    symbol: SymbolHandle,
    is_present: bool,
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
    has_receiver: bool,
    target_state: &Identifier,
) -> Option<ResolvedStateCall> {
    if !has_receiver || receiver_symbol == machine.symbol {
        let is_self_call = has_receiver && receiver_symbol == machine.symbol;

        if let Some(key) =
            resolve_state_key_in_machine(control_flow, machine.symbol, target_symbol, target_state)
        {
            // A `self.X()` that resolves to the CURRENT state is a degenerate
            // self-call: the internal state is a trampoline whose intent is the
            // sibling method (a machine attached to the same data type) of the
            // same name -- internal states are entered by a bare transition
            // `-> X()`, never by `self.X()`. Bind the sibling machine instead so
            // it is not (mis)read as recursion.
            if is_self_call
                && key == source_key
                && let Some(machine_key) =
                    resolve_attached_machine_state_key(control_flow, machine, target_symbol)
            {
                return Some(ResolvedStateCall {
                    key: machine_key,
                    resolution: StateCallResolution::NamedMachine,
                });
            }

            return Some(ResolvedStateCall {
                key,
                resolution: StateCallResolution::Local,
            });
        }

        if is_self_call {
            return resolve_attached_machine_state_key(control_flow, machine, target_symbol).map(
                |key| ResolvedStateCall {
                    key,
                    resolution: StateCallResolution::NamedMachine,
                },
            );
        }

        return None;
    }

    if let Some(contained) = control_flow
        .machine_contains(machine)
        .iter()
        .find(|contained| receiver_symbol.is_valid() && contained.symbol == receiver_symbol)
    {
        if let Some(key) =
            resolve_attached_data_state_key(control_flow, &contained.type_name, target_symbol)
        {
            return Some(ResolvedStateCall {
                key,
                resolution: StateCallResolution::ContainedMachine,
            });
        }

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

        let type_symbol =
            source_state_parameter_machine_symbol(control_flow, source_key, receiver_symbol);
        if type_symbol.is_valid()
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

    if !has_receiver {
        return None;
    }
    let _ = target_state;
    None
}

fn resolve_attached_machine_state_key(
    control_flow: &ControlFlowPlan,
    source_machine: &MachineFlow,
    target_symbol: SymbolHandle,
) -> Option<StateKey> {
    let attached_data = source_machine.attached_data.as_ref()?;
    resolve_attached_data_state_key(control_flow, attached_data, target_symbol)
}

fn resolve_attached_data_state_key(
    control_flow: &ControlFlowPlan,
    attached_data: &Identifier,
    target_symbol: SymbolHandle,
) -> Option<StateKey> {
    if !target_symbol.is_valid() {
        return None;
    }

    control_flow
        .machines
        .iter()
        .filter_map(|(_, candidate)| {
            (candidate.attached_data.as_ref() == Some(attached_data)).then_some(candidate)
        })
        .find_map(|candidate| {
            control_flow
                .states
                .span(candidate.states)?
                .iter()
                .find(|state| state.key.state == target_symbol && state.key.segment_index == 0)
                .map(|state| state.key)
        })
}

fn resolve_state_key_in_machine(
    control_flow: &ControlFlowPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    state_name: &Identifier,
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
    has_receiver: bool,
) -> bool {
    if !has_receiver || receiver_symbol == machine.symbol {
        return false;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    if control_flow
        .machine_contains(machine)
        .iter()
        .any(|contained| contained.symbol == receiver_symbol)
    {
        return true;
    }

    let type_symbol =
        source_state_parameter_machine_symbol(control_flow, source_key, receiver_symbol);
    control_flow.machine_by_symbol(receiver_symbol).is_some()
        || (type_symbol.is_valid() && control_flow.machine_by_symbol(type_symbol).is_some())
}

fn source_state_parameter_machine_symbol(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
) -> SymbolHandle {
    let Some(state) = control_flow
        .states
        .iter()
        .find_map(|(_, state)| (state.key == source_key).then_some(state))
    else {
        return SymbolHandle::invalid();
    };
    control_flow
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| parameter.type_symbol)
        .unwrap_or_else(SymbolHandle::invalid)
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
    use std::sync::Arc;

    #[test]
    fn collects_contained_assignment_value_call() {
        let source = r#"
            data Reward { gold: i32; }
            data Random {}
            data Main { rng: Random; }

            machine Random::one -> i32 {
                pub entry(&mut self) {
                    -> 1;
                }
            }

            machine Main::main(&mut self, reward: &mut Reward) {
                reward.gold = 1 + self.rng.one();
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let entry_key = control_flow
            .states
            .iter()
            .find_map(|(_, state)| {
                let operations = control_flow.operations.span(state.operations)?;
                operations
                    .iter()
                    .any(|operation| {
                        matches!(
                            operation.expressions,
                            omega_control_flow::OperationExpressionRefs::Assignment { .. }
                        )
                    })
                    .then_some(state.key)
            })
            .expect("state with assignment");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let context = StateCallPlanningContext {
            control_flow: Arc::new(control_flow.clone()),
            host_calls: Arc::new(host_calls),
            runtime_flow: Arc::new(runtime_flow),
        };
        let machine = control_flow
            .machine_by_symbol(entry_key.machine)
            .expect("machine flow");

        let calls = collect_machine_state_calls(&context, machine);
        assert!(
            calls.iter().any(|call| {
                call.role == StateCallRole::AssignmentValue
                    && call.source_key.machine == entry_key.machine
                    && call.source_key.state == entry_key.state
                    && call.statement_index == 0
                    && call.receiver_symbol.is_valid()
                    && call.resolution == StateCallResolution::ContainedMachine
            }),
            "expected contained assignment-value call, got {calls:?}"
        );
    }

    #[test]
    fn collects_local_initializer_assignment_value_call() {
        let source = r#"
            data Random {}
            data Main { rng: Random; }

            machine Random::one -> i32 {
                pub entry(&mut self) {
                    -> 1;
                }
            }

            machine Main::main(&mut self) {
                let value: i32 = self.rng.one();
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed).expect("check");
        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let entry_key = control_flow
            .states
            .iter()
            .find_map(|(_, state)| {
                let operations = control_flow.operations.span(state.operations)?;
                operations
                    .iter()
                    .any(|operation| {
                        matches!(operation.kind, omega_control_flow::OperationKind::LocalData)
                            && matches!(
                                operation.expressions,
                                omega_control_flow::OperationExpressionRefs::Expression(_)
                            )
                    })
                    .then_some(state.key)
            })
            .expect("state with initialized local");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let context = StateCallPlanningContext {
            control_flow: Arc::new(control_flow.clone()),
            host_calls: Arc::new(host_calls),
            runtime_flow: Arc::new(runtime_flow),
        };
        let machine = control_flow
            .machine_by_symbol(entry_key.machine)
            .expect("machine flow");

        let calls = collect_machine_state_calls(&context, machine);
        assert!(
            calls.iter().any(|call| {
                call.role == StateCallRole::AssignmentValue
                    && call.source_key.machine == entry_key.machine
                    && call.source_key.state == entry_key.state
                    && call.statement_index == 0
                    && call.receiver_symbol.is_valid()
                    && call.resolution == StateCallResolution::ContainedMachine
            }),
            "expected local-initializer assignment-value call, got {calls:?}"
        );
    }
}
