use super::context::RuntimeDispatchBodyContext;
use super::lookups::{
    host_call_for_statement, local_storage_for_statement, mutation_for_statement,
    state_assignment_value_call, state_call_for_statement, state_has_no_transitions,
    state_operations,
};
use super::model::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_control_flow::{OperationKind, StateKey};
use omega_core::arena::Arena;
use omega_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_state_calls::{StateCall, StateCallLowering};
use omega_state_dispatch::DispatchState;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::expression::ExpressionTable;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<ProgramName>,
    pub operations: Vec<RuntimeDispatchBodyOperation>,
}

pub(super) fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let mut operations = Vec::new();
    let mut expressions = ExpressionTable::new();
    let mut invariant_names = Arena::new();
    append_state_body_operations(
        context,
        dispatch_state.key,
        &mut operations,
        &mut expressions,
        &mut invariant_names,
        &mut Vec::new(),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        dispatch_index: dispatch_state.dispatch_index,
        expressions,
        invariant_names,
        operations,
    }
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<ProgramName>,
    visiting: &mut Vec<StateKey>,
) {
    if visiting.contains(&state_key) {
        return;
    }
    visiting.push(state_key);

    let Some(state_operations) = state_operations(context, state_key) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        if let Some(host_call) =
            host_call_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::HostCall {
                    platform_call: host_call.platform_call.clone(),
                },
            ));
            continue;
        }

        if let Some(state_call) =
            state_call_for_statement(context, state_key, operation.statement_index)
        {
            append_state_call_body_operation(
                context,
                state_call,
                operations,
                expressions,
                invariant_names,
                visiting,
            );
            continue;
        }

        if let Some(state_call) =
            state_assignment_value_call(context, state_key, operation.statement_index)
        {
            append_state_call_body_operation(
                context,
                state_call,
                operations,
                expressions,
                invariant_names,
                visiting,
            );
        }

        if let Some(local_storage) =
            local_storage_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::LocalStorage {
                    symbol: local_storage.symbol,
                    name: local_storage.name.clone(),
                    type_symbol: local_storage.type_symbol,
                    type_name: local_storage.type_name.clone(),
                    invariant_names: invariant_names.insert_many(
                        context
                            .state_storage
                            .invariant_names
                            .span_or_empty(local_storage.invariant_names)
                            .iter()
                            .cloned(),
                    ),
                },
            ));
            continue;
        }

        if let Some(mutation) =
            mutation_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Mutation {
                    mutation_kind: mutation.mutation_kind,
                    lowering: mutation.lowering,
                },
            ));
            continue;
        }

        if !matches!(operation.kind, OperationKind::LocalData) {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Other,
            ));
        }
    }

    for (_, state_call) in context.state_calls.calls.iter() {
        if state_call.source_key == state_key
            && matches!(
                state_call.role,
                omega_state_calls::StateCallRole::TransitionArgument
                    | omega_state_calls::StateCallRole::TransitionGuard
            )
        {
            append_state_call_body_operation(
                context,
                state_call,
                operations,
                expressions,
                invariant_names,
                visiting,
            );
        }
    }

    visiting.pop();
}

fn append_state_call_body_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<ProgramName>,
    visiting: &mut Vec<StateKey>,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.push(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                role: state_call.role,
                call_ordinal: state_call.call_ordinal,
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            operations,
            expressions,
            invariant_names,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    if state_has_no_transitions(context, state_call.target_key) {
        operations.push(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineStateCall {
                role: state_call.role,
                call_ordinal: state_call.call_ordinal,
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
                lowering: state_call.lowering,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            operations,
            expressions,
            invariant_names,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    operations.push(body_operation(
        state_call.source_key,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCall {
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    ));
}

fn append_state_call_result_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
) {
    if !matches!(
        state_call.role,
        omega_state_calls::StateCallRole::AssignmentValue
            | omega_state_calls::StateCallRole::TransitionArgument
            | omega_state_calls::StateCallRole::TransitionGuard
    ) {
        return;
    }

    let Some(value) = terminal_state_value_expression(context, state_call.target_key) else {
        return;
    };
    let value = expressions.copy_from(&context.program.expression_table, value);

    operations.push(body_operation(
        state_call.source_key,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCallResult {
            role: state_call.role,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            value,
        },
    ));
}

fn terminal_state_value_expression(
    context: &RuntimeDispatchBodyContext,
    target_key: StateKey,
) -> Option<omega_checked_trees::expression::ExpressionHandle> {
    let machine = context
        .program
        .machines
        .iter()
        .find(|machine| machine.symbol == target_key.machine)?;
    let state = machine
        .states
        .iter()
        .find(|state| state.symbol == target_key.state)?;
    let statements = context.program.statement_table.statements(state.statement_nodes);
    let statement = statements.last()?;
    match statement {
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Transition(transition)
            if !transition.continuation.is_valid()
                && matches!(transition.guard, TransitionGuardNode::Always) =>
        {
            match context.program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(expression) => Some(*expression),
                _ => None,
            }
        }
        _ => None,
    }
}

fn body_operation(
    source_key: StateKey,
    statement_index: usize,
    kind: RuntimeDispatchBodyOperationKind,
) -> RuntimeDispatchBodyOperation {
    RuntimeDispatchBodyOperation {
        source_key,
        statement_index,
        kind,
    }
}
