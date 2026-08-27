use crate::StateCallPlanningContext;
use omega_control_flow::{
    ControlFlowPlan, MachineFlow, OperationExpressionRefs, OperationKind, PlannedTransitionTarget,
    StateKey, TransitionExpressionRefs,
};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use psi_checked_trees::name::Identifier;
use psi_symbols::SymbolHandle;

use super::{
    StateCallDynamicDispatch, StateCallDynamicDispatchCandidate, StateCallDynamicReceiver,
    StateCallResolution, StateCallRole,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectedStateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub role: StateCallRole,
    pub receiver_symbol: SymbolHandle,
    /// The receiver's spelled field/local name (`b` in `self.b.increment()`).
    /// Symbol handles cross arenas between control flow and layout, so
    /// downstream layout matching (the contained-receiver blocker) keys on
    /// this name; empty when the call has no named receiver.
    pub receiver_name: Identifier,
    /// The receiver EXPRESSION for value-position calls -- the plan build
    /// walks its member chain into `receiver_path`. Invalid for
    /// statement-position calls: their control-flow op carries only the leaf
    /// name, so those fall back to a single-segment path.
    pub raw_receiver: ExpressionHandle,
    /// Exact source-place path retained by a local dynamic coercion. When
    /// present, whole-artifact devirtualization routes `self` through this
    /// place and ignores `raw_receiver`, which names the erased descriptor.
    pub resolved_receiver_path: Vec<Identifier>,
    pub target_key: StateKey,
    pub raw_arguments: HandleSpan<ExpressionHandle>,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
    pub dynamic_dispatch: Option<StateCallDynamicDispatch>,
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
                receiver,
                target,
            } = &operation.kind
            {
                if context
                    .state_statement_has_host_call_by_key(state.key, operation.statement_index)
                {
                    // The statement-position call itself belongs to the host-call
                    // plan, but authored machine calls nested in its arguments still
                    // need state-call sequencing and result storage.
                    collect_expression_state_calls_for_operation(
                        context,
                        machine,
                        state.key,
                        operation.statement_index,
                        &mut call_ordinal,
                        operation.expressions,
                        &mut calls,
                    );
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

                let dynamic_dispatch = resolved_target.is_none().then(|| {
                    resolve_dynamic_dispatch(
                        &context.control_flow,
                        state.key,
                        operation.statement_index,
                        *receiver_symbol,
                        receiver,
                        *target_symbol,
                    )
                });
                if let Some(DynamicDispatchResolution::Resolved(candidate, dispatch)) =
                    dynamic_dispatch
                {
                    calls.push(CollectedStateCall {
                        source_key: state.key,
                        statement_index: operation.statement_index,
                        call_ordinal,
                        role: StateCallRole::Statement,
                        receiver_symbol: *receiver_symbol,
                        receiver_name: receiver.clone(),
                        raw_receiver: ExpressionHandle::invalid(),
                        resolved_receiver_path: Vec::new(),
                        target_key: candidate.key,
                        raw_arguments: match operation.expressions {
                            OperationExpressionRefs::Call { arguments } => arguments,
                            _ => HandleSpan::empty(),
                        },
                        reachable: context.runtime_state_is_reachable_by_key(state.key),
                        required: false,
                        resolution: candidate.resolution,
                        dynamic_dispatch: Some(dispatch),
                    });
                    call_ordinal += 1;
                    collect_expression_state_calls_for_operation(
                        context,
                        machine,
                        state.key,
                        operation.statement_index,
                        &mut call_ordinal,
                        operation.expressions,
                        &mut calls,
                    );
                    continue;
                }

                let rejected_dynamic_dispatch =
                    matches!(dynamic_dispatch, Some(DynamicDispatchResolution::Rejected));
                if rejected_dynamic_dispatch {
                    calls.push(CollectedStateCall {
                        source_key: state.key,
                        statement_index: operation.statement_index,
                        call_ordinal,
                        role: StateCallRole::Statement,
                        receiver_symbol: *receiver_symbol,
                        receiver_name: receiver.clone(),
                        raw_receiver: ExpressionHandle::invalid(),
                        resolved_receiver_path: Vec::new(),
                        target_key: StateKey::default(),
                        raw_arguments: match operation.expressions {
                            OperationExpressionRefs::Call { arguments } => arguments,
                            _ => HandleSpan::empty(),
                        },
                        reachable: context.runtime_state_is_reachable_by_key(state.key),
                        required: false,
                        resolution: StateCallResolution::Unresolved,
                        dynamic_dispatch: None,
                    });
                    call_ordinal += 1;
                    collect_expression_state_calls_for_operation(
                        context,
                        machine,
                        state.key,
                        operation.statement_index,
                        &mut call_ordinal,
                        operation.expressions,
                        &mut calls,
                    );
                    continue;
                }

                // A local or parameter `dyn` selection routes only through
                // exact rows retained by complete closed conformances.
                let dyn_candidates = if resolved_target.is_none() && !rejected_dynamic_dispatch {
                    resolve_dynamic_call_candidates(
                        &context.control_flow,
                        state.key,
                        operation.statement_index,
                        *receiver_symbol,
                        receiver,
                        *target_symbol,
                    )
                } else {
                    Vec::new()
                };
                if !dyn_candidates.is_empty() {
                    let dynamic_receiver = dynamic_source_receiver(
                        &context.control_flow,
                        state.key,
                        operation.statement_index,
                        *receiver_symbol,
                        receiver,
                    );
                    for candidate in dyn_candidates {
                        calls.push(CollectedStateCall {
                            source_key: state.key,
                            statement_index: operation.statement_index,
                            call_ordinal,
                            role: StateCallRole::Statement,
                            receiver_symbol: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.symbol)
                                .unwrap_or(*receiver_symbol),
                            receiver_name: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.name.clone())
                                .unwrap_or_else(|| receiver.clone()),
                            raw_receiver: ExpressionHandle::invalid(),
                            resolved_receiver_path: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.path.clone())
                                .unwrap_or_default(),
                            target_key: candidate.key,
                            raw_arguments: match operation.expressions {
                                OperationExpressionRefs::Call { arguments } => arguments,
                                _ => HandleSpan::empty(),
                            },
                            reachable: context.runtime_state_is_reachable_by_key(state.key),
                            required: false,
                            resolution: candidate.resolution,
                            dynamic_dispatch: None,
                        });
                        call_ordinal += 1;
                    }
                    if !context
                        .state_statement_has_host_call_by_key(state.key, operation.statement_index)
                    {
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
                    continue;
                }

                calls.push(CollectedStateCall {
                    source_key: state.key,
                    statement_index: operation.statement_index,
                    call_ordinal,
                    role: StateCallRole::Statement,
                    receiver_symbol: *receiver_symbol,
                    receiver_name: receiver.clone(),
                    raw_receiver: ExpressionHandle::invalid(),
                    resolved_receiver_path: Vec::new(),
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
                    dynamic_dispatch: None,
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
                &transition.target,
                &transition.continuation,
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
    target: &PlannedTransitionTarget,
    continuation: &PlannedTransitionTarget,
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

    if transition_target_is_named_call(target) {
        *call_ordinal += 1;
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

    if transition_target_is_named_call(continuation) {
        *call_ordinal += 1;
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

fn transition_target_is_named_call(target: &PlannedTransitionTarget) -> bool {
    matches!(
        target,
        PlannedTransitionTarget::State { .. } | PlannedTransitionTarget::Nested { .. }
    )
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
        ExpressionNode::Atomic(atomic) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            role,
            atomic.value,
            calls,
        ),
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
            let dynamic_dispatch = resolved_target.is_none().then(|| {
                resolve_dynamic_dispatch(
                    &context.control_flow,
                    source_key,
                    statement_index,
                    receiver.symbol,
                    &receiver.name,
                    call.target_symbol,
                )
            });
            if let Some(DynamicDispatchResolution::Resolved(candidate, dispatch)) = dynamic_dispatch
            {
                calls.push(CollectedStateCall {
                    source_key,
                    statement_index,
                    call_ordinal: *call_ordinal,
                    role,
                    receiver_symbol: receiver.symbol,
                    receiver_name: receiver.name.clone(),
                    raw_receiver: call.receiver,
                    resolved_receiver_path: Vec::new(),
                    target_key: candidate.key,
                    raw_arguments: call.arguments,
                    reachable: context.runtime_state_is_reachable_by_key(source_key),
                    required: false,
                    resolution: candidate.resolution,
                    dynamic_dispatch: Some(dispatch),
                });
                *call_ordinal += 1;
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
            if matches!(dynamic_dispatch, Some(DynamicDispatchResolution::Rejected)) {
                calls.push(CollectedStateCall {
                    source_key,
                    statement_index,
                    call_ordinal: *call_ordinal,
                    role,
                    receiver_symbol: receiver.symbol,
                    receiver_name: receiver.name.clone(),
                    raw_receiver: call.receiver,
                    resolved_receiver_path: Vec::new(),
                    target_key: StateKey::default(),
                    raw_arguments: call.arguments,
                    reachable: context.runtime_state_is_reachable_by_key(source_key),
                    required: false,
                    resolution: StateCallResolution::Unresolved,
                    dynamic_dispatch: None,
                });
                *call_ordinal += 1;
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
            // A method call through a multi-conformance `dyn Trait` reference
            // parameter has no single target. Retain one exact row candidate
            // per complete conformance; the concrete call-site receiver
            // selects among their inline expansions during selection.
            if resolved_target.is_none()
                && !matches!(dynamic_dispatch, Some(DynamicDispatchResolution::Rejected))
            {
                let candidates = resolve_dynamic_call_candidates(
                    &context.control_flow,
                    source_key,
                    statement_index,
                    receiver.symbol,
                    &receiver.name,
                    call.target_symbol,
                );
                if !candidates.is_empty() {
                    let dynamic_receiver = dynamic_source_receiver(
                        &context.control_flow,
                        source_key,
                        statement_index,
                        receiver.symbol,
                        &receiver.name,
                    );
                    for candidate in candidates {
                        calls.push(CollectedStateCall {
                            source_key,
                            statement_index,
                            call_ordinal: *call_ordinal,
                            role,
                            receiver_symbol: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.symbol)
                                .unwrap_or(receiver.symbol),
                            receiver_name: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.name.clone())
                                .unwrap_or_else(|| receiver.name.clone()),
                            raw_receiver: call.receiver,
                            resolved_receiver_path: dynamic_receiver
                                .as_ref()
                                .map(|receiver| receiver.path.clone())
                                .unwrap_or_default(),
                            target_key: candidate.key,
                            raw_arguments: call.arguments,
                            reachable: context.runtime_state_is_reachable_by_key(source_key),
                            required: false,
                            resolution: candidate.resolution,
                            dynamic_dispatch: None,
                        });
                        *call_ordinal += 1;
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
            }
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
                receiver_name: receiver.name.clone(),
                raw_receiver: call.receiver,
                resolved_receiver_path: Vec::new(),
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
                dynamic_dispatch: None,
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
        ExpressionNode::Borrow(inner) => collect_expression_state_calls_in_table(
            context,
            machine,
            source_key,
            statement_index,
            call_ordinal,
            role,
            inner.target,
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
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

/// Append the receiver's spelled member path (root -> leaf) to `segments`,
/// growing `span` contiguously: `self.p.second.stored()` appends `["self",
/// "p", "second"]`. MIRRORS `call_receiver_parts` below -- the last appended
/// segment always equals that function's leaf `name` (keep the two walks in
/// lockstep). Non-place receivers (calls, literals) append nothing.
pub(crate) fn append_receiver_path(
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    segments: &mut Arena<Identifier>,
    span: &mut HandleSpan<Identifier>,
) {
    if !receiver.is_valid() {
        return;
    }

    match expressions.expression(receiver) {
        ExpressionNode::Borrow(inner) => {
            append_receiver_path(expressions, inner.target, segments, span);
        }
        ExpressionNode::Name(path) => {
            if let Some(name) = expressions.name_path_members(path.members).last() {
                span.push_contiguous(segments.insert(name.clone()));
            }
        }
        ExpressionNode::Member(member) => {
            append_receiver_path(expressions, member.receiver, segments, span);
            span.push_contiguous(segments.insert(member.member.clone()));
        }
        _ => {}
    }
}

fn call_receiver_parts(expressions: &ExpressionTable, receiver: ExpressionHandle) -> ReceiverParts {
    if !receiver.is_valid() {
        return ReceiverParts::default();
    }

    match expressions.expression(receiver) {
        ExpressionNode::Borrow(inner) => call_receiver_parts(expressions, inner.target),
        ExpressionNode::Name(path) => ReceiverParts {
            symbol: path.symbol,
            name: expressions
                .name_path_members(path.members)
                .last()
                .cloned()
                .unwrap_or_default(),
            is_present: true,
        },
        ExpressionNode::Member(member) => {
            let mut receiver = call_receiver_parts(expressions, member.receiver);
            receiver.symbol = member.member_symbol;
            receiver.name = member.member.clone();
            receiver.is_present = true;
            receiver
        }
        _ => ReceiverParts::default(),
    }
}

#[derive(Debug, Clone, Default)]
struct ReceiverParts {
    symbol: SymbolHandle,
    name: Identifier,
    is_present: bool,
}

fn dynamic_source_receiver(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    statement_index: usize,
    binding: SymbolHandle,
    binding_name: &Identifier,
) -> Option<DynamicSourceReceiver> {
    let selection = control_flow
        .semantics
        .facts
        .dynamic_conformances
        .for_receiver(
            source_key.machine,
            source_key.state,
            binding,
            binding_name,
            statement_index,
        )?;
    Some(DynamicSourceReceiver {
        symbol: selection.source_symbol,
        name: selection.source_name.clone(),
        path: selection.source_path.clone(),
    })
}

#[derive(Debug, Clone)]
struct DynamicSourceReceiver {
    symbol: SymbolHandle,
    name: Identifier,
    path: Vec<Identifier>,
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

        // Monomorphization rewrites a call through a static machine parameter
        // (`Before(left, right)`) to the exact ENTRY-state symbol selected at
        // the call site.  The authored target spelling remains the parameter's
        // leaf name, so neither current-machine nor free-machine name lookup
        // can recover an attached/named satisfier such as `F64::TotalOrder`.
        // The resolved state symbol is globally unique and is therefore the
        // authoritative receiverless dispatch identity.
        if !has_receiver
            && target_symbol.is_valid()
            && let Some(state) = control_flow.states.iter().find_map(|(_, state)| {
                (state.key.state == target_symbol && state.key.segment_index == 0).then_some(state)
            })
        {
            return Some(ResolvedStateCall {
                key: state.key,
                resolution: StateCallResolution::NamedMachine,
            });
        }

        // A receiverless call whose target is a FREE top-level machine
        // (`machine pick(x: i32) -> i32 { ... }`, called as `pick(self.v)`):
        // resolve to that machine's entry state so the call is collected (and
        // inline-branched/dispatched) like a method call. Without this the call
        // was invisible to state-call planning and a value-position `let n =
        // pick(..)` silently left `n` at 0.
        if !has_receiver
            && let Some(target_machine) =
                resolve_free_machine(control_flow, target_symbol, target_state)
            && target_machine.symbol != machine.symbol
        {
            // A free machine's single body state is its entry (named `entry`,
            // not after the machine), so fall back to the machine's first
            // root-segment state when the symbol/name lookup misses.
            return resolve_state_key_in_machine(
                control_flow,
                target_machine.symbol,
                target_symbol,
                target_state,
            )
            .or_else(|| {
                control_flow
                    .states
                    .span(target_machine.states)?
                    .iter()
                    .find(|state| state.key.segment_index == 0)
                    .map(|state| state.key)
            })
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::NamedMachine,
            });
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

        // The frontend target symbol can name the same method on a sibling
        // specialization (for example `Cell<i32>::get` at a
        // `Cell<bool>::get` site). The contained receiver's concrete type is
        // authoritative; fall back to the method spelling across every
        // machine attached to that exact type before consulting the one
        // representative machine symbol stored in `ContainedGraph`.
        if let Some(key) = resolve_attached_data_state_key_by_name(
            control_flow,
            &contained.type_name,
            target_symbol,
            target_state,
        ) {
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

        // A method call on a reference PARAMETER of a DATA type (`s: &mut Circle`,
        // `s.code()`): the param's type is a data type, not a machine, so resolve
        // the method in the machine ATTACHED to that data type -- the same way a
        // contained-object call (`self.c.code()`) resolves. Also covers a
        // devirtualized `dyn Trait` param (its type resolves to the impl's data type).
        if let Some(type_name) =
            source_state_parameter_type_name(control_flow, source_key, receiver_symbol)
            && let Some(key) = resolve_attached_data_state_key_by_name(
                control_flow,
                &type_name,
                target_symbol,
                target_state,
            )
        {
            return Some(ResolvedStateCall {
                key,
                resolution: StateCallResolution::ContainedMachine,
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

/// Resolve a dynamic receiver exclusively from checked closed-conformance
/// rows. A local coercion carries one exact selection; a bare parameter carries
/// every eligible complete conformance. Carrier and requirement spellings are
/// never used to rediscover an implementation.
fn resolve_dynamic_call_candidates(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    statement_index: usize,
    receiver_symbol: SymbolHandle,
    receiver_name: &Identifier,
    target_symbol: SymbolHandle,
) -> Vec<ResolvedStateCall> {
    if !receiver_symbol.is_valid() && receiver_name.as_str().is_empty() {
        return Vec::new();
    }
    if let Some(selection) = control_flow
        .semantics
        .facts
        .dynamic_conformances
        .for_receiver(
            source_key.machine,
            source_key.state,
            receiver_symbol,
            receiver_name,
            statement_index,
        )
    {
        return selection
            .rows
            .iter()
            .filter(|row| row.requirement == target_symbol)
            .filter_map(|row| {
                control_flow
                    .states
                    .iter()
                    .find(|(_, state)| {
                        state.key.state == row.realization_state && state.key.segment_index == 0
                    })
                    .map(|(_, state)| ResolvedStateCall {
                        key: state.key,
                        resolution: StateCallResolution::ContainedMachine,
                    })
            })
            .collect();
    }
    let Some(state) = control_flow
        .states
        .iter()
        .find_map(|(_, state)| (state.key == source_key).then_some(state))
    else {
        return Vec::new();
    };
    let Some(parameter) = control_flow
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
    else {
        return Vec::new();
    };
    if !parameter.dyn_conformance_rows.is_empty() {
        return parameter
            .dyn_conformance_rows
            .iter()
            .filter(|row| row.requirement == target_symbol)
            .filter_map(|row| {
                control_flow
                    .states
                    .iter()
                    .find(|(_, state)| {
                        state.key.state == row.realization_state && state.key.segment_index == 0
                    })
                    .map(|(_, state)| ResolvedStateCall {
                        key: state.key,
                        resolution: StateCallResolution::ContainedMachine,
                    })
            })
            .collect();
    }
    parameter
        .dyn_conformance_candidates
        .iter()
        .flat_map(|candidate| candidate.rows.iter())
        .filter(|row| row.requirement == target_symbol)
        .filter_map(|row| {
            control_flow
                .states
                .iter()
                .find(|(_, state)| {
                    state.key.state == row.realization_state && state.key.segment_index == 0
                })
                .map(|(_, state)| ResolvedStateCall {
                    key: state.key,
                    resolution: StateCallResolution::ContainedMachine,
                })
        })
        .collect()
}

enum DynamicDispatchResolution {
    NotApplicable,
    Resolved(ResolvedStateCall, StateCallDynamicDispatch),
    Rejected,
}

fn resolve_dynamic_dispatch(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    statement_index: usize,
    receiver_symbol: SymbolHandle,
    receiver_name: &Identifier,
    target_symbol: SymbolHandle,
) -> DynamicDispatchResolution {
    let matching_selections = control_flow
        .semantics
        .facts
        .dynamic_conformances
        .selections
        .iter()
        .filter(|selection| {
            selection.machine == source_key.machine
                && selection.state == source_key.state
                && selection.statement_index < statement_index
                && if receiver_symbol.is_valid() {
                    selection.binding == receiver_symbol
                } else {
                    selection.binding_name == *receiver_name
                }
        })
        .collect::<Vec<_>>();
    if matching_selections.len() > 1 {
        let Some(binding) = matching_selections
            .first()
            .map(|selection| selection.binding)
            .filter(|binding| binding.is_valid())
        else {
            return DynamicDispatchResolution::Rejected;
        };
        if receiver_symbol.is_valid() && receiver_symbol != binding {
            return DynamicDispatchResolution::Rejected;
        }
        if matching_selections
            .iter()
            .any(|selection| selection.binding != binding)
        {
            return DynamicDispatchResolution::Rejected;
        }
        let Some(selection) = matching_selections
            .iter()
            .max_by_key(|selection| selection.statement_index)
            .copied()
        else {
            return DynamicDispatchResolution::Rejected;
        };
        if matching_selections.iter().any(|version| {
            version.target_trait != selection.target_trait
                || version.conformance != selection.conformance
                || version.source_data != selection.source_data
                || version.rows != selection.rows
        }) {
            return DynamicDispatchResolution::Rejected;
        }
        let Some(conformance) = selection.conformance else {
            return DynamicDispatchResolution::Rejected;
        };
        let mut matching_rows = selection
            .rows
            .iter()
            .filter(|row| row.requirement == target_symbol);
        let Some(row) = matching_rows.next() else {
            return DynamicDispatchResolution::Rejected;
        };
        if matching_rows.next().is_some() || row.declaring_trait != selection.target_trait {
            return DynamicDispatchResolution::Rejected;
        }
        let mut states = control_flow.states.iter().filter(|(_, state)| {
            state.key.machine == row.realization_machine
                && state.key.state == row.realization_state
                && state.key.segment_index == 0
        });
        let Some(state) = states.next().map(|(_, state)| state) else {
            return DynamicDispatchResolution::Rejected;
        };
        if states.next().is_some() {
            return DynamicDispatchResolution::Rejected;
        }
        return DynamicDispatchResolution::Resolved(
            ResolvedStateCall {
                key: state.key,
                resolution: StateCallResolution::ContainedMachine,
            },
            StateCallDynamicDispatch {
                receiver: StateCallDynamicReceiver::ReboundLocal {
                    binding,
                    selection_statement_index: selection.statement_index,
                },
                target_trait: selection.target_trait,
                requirement: target_symbol,
                requirement_identity: row.requirement_identity.clone(),
                candidates: vec![StateCallDynamicDispatchCandidate {
                    source_data: selection.source_data,
                    conformance,
                    rows: selection.rows.clone(),
                }],
            },
        );
    }

    let Some(state) = control_flow
        .states
        .iter()
        .find_map(|(_, state)| (state.key == source_key).then_some(state))
    else {
        return DynamicDispatchResolution::NotApplicable;
    };
    let Some(parameter) = control_flow
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
    else {
        return DynamicDispatchResolution::NotApplicable;
    };
    if parameter.dyn_conformance_candidates.is_empty() {
        return DynamicDispatchResolution::NotApplicable;
    }

    let mut requirement_identity: Option<String> = None;
    let mut representative = None;
    let mut candidates = Vec::with_capacity(parameter.dyn_conformance_candidates.len());
    for candidate in &parameter.dyn_conformance_candidates {
        let Some(conformance) = candidate.conformance else {
            return DynamicDispatchResolution::Rejected;
        };
        let mut matching_rows = candidate
            .rows
            .iter()
            .filter(|row| row.requirement == target_symbol);
        let Some(row) = matching_rows.next() else {
            return DynamicDispatchResolution::Rejected;
        };
        if matching_rows.next().is_some() || row.declaring_trait != parameter.type_symbol {
            return DynamicDispatchResolution::Rejected;
        }
        match &requirement_identity {
            Some(identity) if identity != &row.requirement_identity => {
                return DynamicDispatchResolution::Rejected;
            }
            None => requirement_identity = Some(row.requirement_identity.clone()),
            _ => {}
        }
        let mut states = control_flow.states.iter().filter(|(_, state)| {
            state.key.machine == row.realization_machine
                && state.key.state == row.realization_state
                && state.key.segment_index == 0
        });
        let Some(state) = states.next().map(|(_, state)| state) else {
            return DynamicDispatchResolution::Rejected;
        };
        if states.next().is_some() {
            return DynamicDispatchResolution::Rejected;
        }
        representative.get_or_insert(ResolvedStateCall {
            key: state.key,
            resolution: StateCallResolution::ContainedMachine,
        });
        candidates.push(StateCallDynamicDispatchCandidate {
            source_data: candidate.source_data,
            conformance,
            rows: candidate.rows.clone(),
        });
    }

    let (Some(representative), Some(requirement_identity)) = (representative, requirement_identity)
    else {
        return DynamicDispatchResolution::Rejected;
    };
    DynamicDispatchResolution::Resolved(
        representative,
        StateCallDynamicDispatch {
            receiver: StateCallDynamicReceiver::Parameter {
                symbol: parameter.symbol,
            },
            target_trait: parameter.type_symbol,
            requirement: target_symbol,
            requirement_identity,
            candidates,
        },
    )
}

/// A FREE top-level machine (`machine pick(x: i32) -> i32`, no attached data)
/// matched by the call's target symbol, or -- because the frontend leaves a
/// receiverless free-machine call's `target_symbol` unresolved -- by NAME among
/// machines with no attached data (a receiverless call can only target a local
/// state, already tried by the caller, or a free machine; attached methods
/// always carry a receiver, so same-named methods cannot collide here).
fn resolve_free_machine<'plan>(
    control_flow: &'plan ControlFlowPlan,
    target_symbol: SymbolHandle,
    target_state: &Identifier,
) -> Option<&'plan MachineFlow> {
    if target_symbol.is_valid() {
        let exact = control_flow
            .machines
            .iter()
            .map(|(_, machine)| machine)
            .find(|machine| {
                control_flow
                    .states
                    .span(machine.states)
                    .is_some_and(|states| {
                        states.iter().any(|state| state.key.state == target_symbol)
                    })
            });
        if exact.is_some() {
            return exact;
        }
    }

    control_flow
        .machines
        .iter()
        .map(|(_, machine)| machine)
        .find(|machine| machine.attached_data.is_none() && machine.name == *target_state)
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

/// Like [`resolve_attached_data_state_key`] but also matches the target by NAME,
/// not just symbol. A method call on a reference param (`s.code()`) may carry the
/// trait/declared method symbol rather than the concrete impl's state symbol, so
/// name matching is the reliable fallback (the type checker already validated it).
fn resolve_attached_data_state_key_by_name(
    control_flow: &ControlFlowPlan,
    attached_data: &Identifier,
    target_symbol: SymbolHandle,
    target_state: &Identifier,
) -> Option<StateKey> {
    let candidates = control_flow
        .machines
        .iter()
        .filter_map(|(_, candidate)| {
            (candidate.attached_data.as_ref() == Some(attached_data)).then_some(candidate)
        })
        .collect::<Vec<_>>();
    if target_symbol.is_valid()
        && let Some(exact) = candidates.iter().find_map(|candidate| {
            control_flow
                .states
                .span(candidate.states)?
                .iter()
                .find(|state| state.key.state == target_symbol)
                .map(|state| state.key)
        })
    {
        return Some(exact);
    }
    candidates.into_iter().find_map(|candidate| {
        control_flow
            .states
            .span(candidate.states)?
            .iter()
            .find(|state| state.key.segment_index == 0 && state.name == *target_state)
            .map(|state| state.key)
    })
}

fn source_state_parameter_type_name(
    control_flow: &ControlFlowPlan,
    source_key: StateKey,
    receiver_symbol: SymbolHandle,
) -> Option<Identifier> {
    let state = control_flow
        .states
        .iter()
        .find_map(|(_, state)| (state.key == source_key).then_some(state))?;
    control_flow
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| parameter.type_name.clone())
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
    use omega_state_graph::build_runtime_flow_plan;
    use omega_state_graph_to_control_flow::build_control_flow_plan;
    use psi_source_files_to_tokens::Lexer;
    use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use psi_tokens_to_syntax_trees::parse_syntax_trees;
    use psi_typed_trees_to_checked_trees::lower_typed_trees;
    use std::sync::Arc;

    #[test]
    fn collects_contained_assignment_value_call() {
        let source = r#"
            data Reward { gold: i32; }
            data Random {}
            data Main { rng: Random; }

            machine Random::one(&mut self) -> i32 {
                transition { _ -> 1 }
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

            machine Random::one(&mut self) -> i32 {
                transition { _ -> 1 }
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

    #[test]
    fn closed_dynamic_parameter_call_consumes_the_retained_exact_row() {
        let source = r#"
            trait Shape {
                machine code(&self) -> i32;
            }
            data Item {}
            machine Item::code(&self) -> i32 {
                transition { _ -> 1 }
            }
            Primary: Item satisfies Shape {
                machine code(&self) -> i32 {
                    transition { _ -> 7 }
                }
            }

            machine run(erased: &dyn Item::Primary) {
                let result: i32 = erased.code();
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate");
        let checked = lower_typed_trees(typed).expect("check");
        let exact_target = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Item::Primary::code")
            .and_then(|machine| checked.machine_states(machine).first())
            .map(|state| state.symbol)
            .expect("exact selected row state");
        let ambient_target = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Item::code")
            .and_then(|machine| checked.machine_states(machine).first())
            .map(|state| state.symbol)
            .expect("ambient look-alike state");
        assert_ne!(exact_target, ambient_target);

        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let machine = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry_key = control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run state");
        let [parameter] = control_flow
            .state_by_key(entry_key)
            .map(|state| control_flow.state_parameters(state))
            .expect("run parameters")
        else {
            panic!("one dynamic parameter");
        };
        let [parameter_row] = parameter.dyn_conformance_rows.as_slice() else {
            panic!("one retained exact parameter row");
        };
        assert_eq!(parameter_row.realization_state, exact_target);
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let context = StateCallPlanningContext {
            control_flow: Arc::new(control_flow.clone()),
            host_calls: Arc::new(host_calls),
            runtime_flow: Arc::new(runtime_flow),
        };

        let calls = collect_machine_state_calls(&context, machine);
        let dynamic_call = calls
            .iter()
            .find(|call| call.target_key.state == exact_target)
            .expect("dynamic call should route to the exact retained row");
        assert_eq!(
            dynamic_call.resolution,
            StateCallResolution::ContainedMachine
        );
        assert_ne!(dynamic_call.target_key.state, ambient_target);
    }

    #[test]
    fn bare_dynamic_parameter_candidates_retain_only_closed_exact_rows() {
        let source = r#"
            trait Shape { machine code(&self) -> i32; }

            data Circle {}
            machine Circle::code(&self) -> i32 { transition { _ -> 1 } }
            CircleShape: Circle satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 7 } }
            }

            data Square {}
            machine Square::code(&self) -> i32 { transition { _ -> 2 } }
            SquareShape: Square satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 9 } }
            }

            machine run(erased: &dyn Shape) {
                let result: i32 = erased.code();
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate");
        let checked = lower_typed_trees(typed).expect("check");
        let exact_targets = ["Circle::CircleShape::code", "Square::SquareShape::code"]
            .into_iter()
            .map(|name| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == name)
                    .and_then(|machine| checked.machine_states(machine).first())
                    .map(|state| state.symbol)
                    .unwrap_or_else(|| panic!("exact state `{name}`"))
            })
            .collect::<Vec<_>>();
        let ambient_targets = ["Circle::code", "Square::code"]
            .into_iter()
            .map(|name| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str() == name)
                    .and_then(|machine| checked.machine_states(machine).first())
                    .map(|state| state.symbol)
                    .unwrap_or_else(|| panic!("ambient state `{name}`"))
            })
            .collect::<Vec<_>>();

        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let machine = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry_key = control_flow
            .states
            .span(machine.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run state");
        let [parameter] = control_flow
            .state_by_key(entry_key)
            .map(|state| control_flow.state_parameters(state))
            .expect("run parameters")
        else {
            panic!("one dynamic parameter");
        };
        assert!(parameter.dyn_conformance_rows.is_empty());
        assert_eq!(parameter.dyn_conformance_candidates.len(), 2);
        let retained_targets = parameter
            .dyn_conformance_candidates
            .iter()
            .flat_map(|candidate| candidate.rows.iter())
            .map(|row| row.realization_state)
            .collect::<Vec<_>>();
        assert_eq!(retained_targets, exact_targets);
        assert!(
            ambient_targets
                .iter()
                .all(|ambient| !retained_targets.contains(ambient)),
            "ambient same-named attached machines must not enter dynamic candidates"
        );

        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let context = StateCallPlanningContext {
            control_flow: Arc::new(control_flow.clone()),
            host_calls: Arc::new(host_calls),
            runtime_flow: Arc::new(runtime_flow),
        };
        let calls = collect_machine_state_calls(&context, machine);
        let dynamic_calls = calls
            .iter()
            .filter(|call| call.role == StateCallRole::AssignmentValue)
            .collect::<Vec<_>>();
        let [dynamic_call] = dynamic_calls.as_slice() else {
            panic!("one representative dynamic call");
        };
        assert!(exact_targets.contains(&dynamic_call.target_key.state));
        let dispatch = dynamic_call
            .dynamic_dispatch
            .as_ref()
            .expect("retained dynamic dispatch");
        assert_eq!(dispatch.candidates.len(), 2);
        let retained_dispatch_targets = dispatch
            .candidates
            .iter()
            .flat_map(|candidate| candidate.rows.iter())
            .map(|row| row.realization_state)
            .collect::<Vec<_>>();
        assert_eq!(retained_dispatch_targets, exact_targets);
    }

    #[test]
    fn local_dynamic_selection_reaches_control_flow_with_stable_owner_coordinates() {
        let source = r#"
            trait Shape { machine code(&self) -> i32; }
            data Item {}
            Primary: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 7 } }
            }
            machine code() -> i32 { transition { _ -> 4 } }
            machine run(item: Item) -> i32 {
                let erased: &dyn Shape = &item as &dyn Item::Primary;
                let result: i32 = erased.code();
                transition { _ -> result }
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate");
        let checked = lower_typed_trees(typed).expect("check");
        let [checked_selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
            panic!("one checked selection");
        };
        assert!(checked_selection.binding.is_valid());
        assert!(checked_selection.machine.is_valid());
        assert!(checked_selection.state.is_valid());
        assert!(checked_selection.source_symbol.is_valid());
        assert_eq!(checked_selection.source_name.as_str(), "item");
        assert_eq!(
            checked_selection
                .source_path
                .iter()
                .map(|segment| segment.as_str())
                .collect::<Vec<_>>(),
            vec!["item"]
        );
        let dynamic_call_target = checked
            .expression_table
            .expression_entries()
            .find_map(|(_, expression)| {
                let ExpressionNode::Call(call) = expression else {
                    return None;
                };
                (call.target.as_str() == "code").then_some(call.target_symbol)
            })
            .expect("typed dynamic call target");
        assert_eq!(
            dynamic_call_target, checked_selection.rows[0].requirement,
            "dynamic call must retain the exact declaring-trait requirement symbol"
        );

        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        assert_eq!(
            state_graph.semantics.facts.dynamic_conformances,
            checked.facts.dynamic_conformances.binding_facts()
        );
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        assert_eq!(
            control_flow.semantics.facts.dynamic_conformances,
            checked.facts.dynamic_conformances.binding_facts()
        );
        let run = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry_key = control_flow
            .states
            .span(run.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run entry");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let state_calls = crate::build_state_call_plan(&control_flow, &host_calls, &runtime_flow);
        let dynamic_call = state_calls
            .calls
            .iter()
            .map(|(_, call)| call)
            .find(|call| call.target_key.state == checked_selection.rows[0].realization_state)
            .expect("selected dynamic call");
        assert_eq!(
            dynamic_call.receiver_symbol,
            checked_selection.source_symbol
        );
        assert_eq!(dynamic_call.receiver_name.as_str(), "item");
        assert_eq!(
            state_calls
                .receiver_path_segments
                .span(dynamic_call.receiver_path)
                .expect("retained source path")
                .iter()
                .map(|segment| segment.as_str())
                .collect::<Vec<_>>(),
            vec!["item"]
        );
    }

    #[test]
    fn selected_dynamic_argument_retains_its_exact_descriptor_rows() {
        let source = r#"
            trait Shape { machine code(&self) -> i32; }
            data Item {}
            First: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 1 } }
            }
            Second: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 2 } }
            }
            machine dispatch(erased: &dyn Shape) -> i32 {
                let result: i32 = erased.code();
                transition { _ -> result }
            }
            machine run(item: Item) -> i32 {
                let erased: &dyn Shape = &item as &dyn Item::First;
                let result: i32 = dispatch(erased);
                transition { _ -> result }
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate selected pass-through");
        let checked = lower_typed_trees(typed).expect("check");
        let [selection] = checked.facts.dynamic_conformances.selections.as_slice() else {
            panic!("one checked local selection");
        };
        let selected_conformance = selection.conformance.expect("named conformance");
        let dispatch = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "dispatch")
            .expect("dispatch machine");

        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        assert_eq!(
            state_graph.semantics.facts.dynamic_conformances,
            checked.facts.dynamic_conformances.binding_facts()
        );
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        assert_eq!(
            control_flow.semantics.facts.dynamic_conformances,
            checked.facts.dynamic_conformances.binding_facts()
        );
        let run = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry_key = control_flow
            .states
            .span(run.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run entry");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let state_calls = crate::build_state_call_plan(&control_flow, &host_calls, &runtime_flow);
        let pass_call = state_calls
            .calls
            .iter()
            .find(|(_, call)| call.target_key.machine == dispatch.symbol)
            .map(|(_, call)| call)
            .expect("run-to-dispatch call");
        let [argument] = state_calls.arguments.span_or_empty(pass_call.arguments) else {
            panic!("one pass-through argument");
        };
        let descriptor = argument
            .dynamic_conformance
            .as_ref()
            .expect("exact forwarded descriptor identity");
        assert_eq!(descriptor.source_binding, selection.binding);
        assert_eq!(descriptor.source_data, selection.source_data);
        assert_eq!(descriptor.target_trait, selection.target_trait);
        assert_eq!(descriptor.conformance, selected_conformance);
        assert_eq!(descriptor.rows, selection.rows);
        assert_eq!(descriptor.rows.len(), 1);
        let row = &descriptor.rows[0];
        let declaring_trait = checked
            .traits()
            .iter()
            .find(|definition| definition.symbol == row.declaring_trait)
            .expect("retained declaring trait");
        let requirement = checked
            .trait_machine_signatures(declaring_trait)
            .iter()
            .find(|requirement| requirement.symbol == row.requirement)
            .expect("retained requirement");
        let realization = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == row.realization_machine)
            .expect("retained realization machine");
        assert_eq!(
            row.requirement_identity,
            checked
                .normalized_trait_requirement_overload_identity(declaring_trait, requirement)
                .identity()
        );
        assert_eq!(
            row.realization_identity,
            checked
                .normalized_machine_overload_identity(realization)
                .expect("normalized realization")
                .identity()
        );
    }

    #[test]
    fn rebound_local_direct_call_retains_exact_indirect_dispatch_identity() {
        let source = r#"
            trait Shape { machine code(&self) -> i32; }
            data Item {}
            Primary: Item satisfies Shape {
                machine code(&self) -> i32 { transition { _ -> 7 } }
            }
            machine run(first: Item, second: Item) -> i32 {
                let mut erased: &dyn Shape = &first as &dyn Item::Primary;
                erased = &second as &dyn Item::Primary;
                let result: i32 = erased.code();
                transition { _ -> result }
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        psi_validation::validate_program(&typed).expect("validate rebound direct call");
        let checked = lower_typed_trees(typed).expect("check");
        let [initial, rebound] = checked.facts.dynamic_conformances.selections.as_slice() else {
            panic!("initial and rebound selections");
        };
        assert_eq!(initial.binding, rebound.binding);

        let state_graph =
            omega_checked_trees_to_state_graph::build_state_graph(&checked).expect("state graph");
        let control_flow = build_control_flow_plan(&state_graph).expect("control flow");
        let run = control_flow
            .machines
            .iter()
            .find(|(_, machine)| machine.name.as_str() == "run")
            .map(|(_, machine)| machine)
            .expect("run machine");
        let entry_key = control_flow
            .states
            .span(run.states)
            .and_then(|states| states.first())
            .map(|state| state.key)
            .expect("run entry");
        let runtime_flow = build_runtime_flow_plan(&control_flow, entry_key).expect("runtime flow");
        let target = omega_target::NativeTarget::linux_arm64();
        let host_abi = build_host_abi_plan(target);
        let host_calls = build_host_call_plan(&checked, target, &host_abi).expect("host calls");
        let calls = crate::build_state_call_plan(&control_flow, &host_calls, &runtime_flow);
        let call = calls
            .calls
            .iter()
            .find(|(_, call)| call.dynamic_dispatch.is_some())
            .map(|(_, call)| call)
            .expect("rebound direct call");
        assert_eq!(call.lowering, crate::StateCallLowering::IndirectDynamic);
        let dispatch = call.dynamic_dispatch.as_ref().expect("dynamic dispatch");
        assert_eq!(
            dispatch.receiver,
            StateCallDynamicReceiver::ReboundLocal {
                binding: rebound.binding,
                selection_statement_index: rebound.statement_index,
            }
        );
        let [candidate] = dispatch.candidates.as_slice() else {
            panic!("one rebound candidate");
        };
        assert_eq!(candidate.source_data, rebound.source_data);
        assert_eq!(
            candidate.conformance,
            rebound.conformance.expect("conformance")
        );
        assert_eq!(candidate.rows, rebound.rows);

        let mut collided = control_flow.clone();
        collided.semantics.facts.dynamic_conformances.selections[1].binding = initial.source_symbol;
        let rejected = crate::build_state_call_plan(&collided, &host_calls, &runtime_flow);
        let rejected_call = rejected
            .calls
            .iter()
            .find(|(_, call)| call.statement_index == rebound.statement_index + 1)
            .map(|(_, call)| call)
            .expect("recognized rebound collision remains an unresolved call");
        assert_eq!(rejected_call.lowering, crate::StateCallLowering::Unresolved);
        assert!(rejected_call.dynamic_dispatch.is_none());
        assert!(!rejected_call.target_key.is_valid());
    }
}
