use super::context::RuntimeDispatchBodyContext;
use super::lookups::{
    host_call_for_statement, local_storage_for_statement, mutation_for_statement,
    state_assignment_value_call, state_call_for_statement, state_has_no_transitions,
    state_operations,
};
use super::model::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_checked_trees::expression::ExpressionTable;
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use omega_checked_trees::types::TypeReferenceTable;
use omega_control_flow::{OperationKind, StateKey};
use omega_core::arena::Arena;
use omega_state_calls::{StateCall, StateCallLowering};
use omega_state_dispatch::DispatchState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub expressions: ExpressionTable,
    pub invariant_names: Arena<ProgramName>,
    pub operations: Arena<RuntimeDispatchBodyOperation>,
    pub type_references: TypeReferenceTable,
}

const INLINE_BODY_VISITING_COUNT: usize = 16;

struct BodyVisitingStates {
    inline: [Option<StateKey>; INLINE_BODY_VISITING_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl BodyVisitingStates {
    fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_BODY_VISITING_COUNT],
            len: 0,
            overflow: Vec::with_capacity(state_capacity.saturating_sub(INLINE_BODY_VISITING_COUNT)),
        }
    }

    fn contains(&self, key: StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_BODY_VISITING_COUNT))
            .flatten()
            .any(|candidate| *candidate == key)
            || self.overflow.contains(&key)
    }

    fn push(&mut self, key: StateKey) {
        if self.len < INLINE_BODY_VISITING_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_BODY_VISITING_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

pub(super) fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let operation_capacity = estimated_body_operation_capacity(context, dispatch_state.key);
    let mut operations = Arena::with_capacity(operation_capacity);
    let mut expressions = ExpressionTable::with_expression_capacity(operation_capacity);
    let mut invariant_names = Arena::with_capacity(estimated_body_invariant_name_capacity(
        context,
        dispatch_state.key,
    ));
    let mut type_references = TypeReferenceTable::new();
    append_state_body_operations(
        context,
        dispatch_state.key,
        &mut operations,
        &mut expressions,
        &mut invariant_names,
        &mut type_references,
        &mut BodyVisitingStates::with_capacity(context.control_flow.states.len()),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        dispatch_index: dispatch_state.dispatch_index,
        expressions,
        invariant_names,
        operations,
        type_references,
    }
}

fn estimated_body_operation_capacity(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> usize {
    state_operations(context, state_key).map_or(0, <[omega_control_flow::Operation]>::len)
        + context
            .state_calls
            .calls
            .iter()
            .filter(|(_, state_call)| state_call.source_key == state_key)
            .count()
}

fn estimated_body_invariant_name_capacity(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> usize {
    context
        .state_storage
        .locals
        .iter()
        .filter(|(_, local)| local.source_key == state_key)
        .map(|(_, local)| local.invariant_names.len())
        .sum()
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<ProgramName>,
    type_references: &mut TypeReferenceTable,
    visiting: &mut BodyVisitingStates,
) {
    if visiting.contains(state_key) {
        return;
    }
    visiting.push(state_key);

    let Some(state_operations) = state_operations(context, state_key) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        if host_call_for_statement(context, state_key, operation.statement_index).is_some() {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::HostCall,
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
                type_references,
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
                type_references,
                visiting,
            );
        }

        if let Some(local_storage) =
            local_storage_for_statement(context, state_key, operation.statement_index)
        {
            operations.insert(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::LocalStorage {
                    symbol: local_storage.symbol,
                    name: local_storage.name.clone(),
                    type_symbol: local_storage.type_symbol,
                    type_reference: type_references.copy_from(
                        &context.state_storage.type_references,
                        &context.state_storage.expressions,
                        expressions,
                        local_storage.type_reference,
                    ),
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
            operations.insert(body_operation(
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
            operations.insert(body_operation(
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
                type_references,
                visiting,
            );
        }
    }

    visiting.pop();
}

fn append_state_call_body_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
    expressions: &mut ExpressionTable,
    invariant_names: &mut Arena<ProgramName>,
    type_references: &mut TypeReferenceTable,
    visiting: &mut BodyVisitingStates,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.insert(body_operation(
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
            type_references,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    if state_has_no_transitions(context, state_call.target_key) {
        operations.insert(body_operation(
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
            type_references,
            visiting,
        );
        append_state_call_result_operation(context, state_call, operations, expressions);
        return;
    }

    operations.insert(body_operation(
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
    operations: &mut Arena<RuntimeDispatchBodyOperation>,
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

    let value = terminal_state_value_expression(context, state_call.target_key);
    if !value.is_valid() {
        return;
    };
    let value = expressions.copy_from(&context.program.expression_table, value);

    operations.insert(body_operation(
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
) -> omega_checked_trees::expression::ExpressionHandle {
    let Some(machine) = context
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_key.machine)
    else {
        return omega_checked_trees::expression::ExpressionHandle::invalid();
    };
    let Some(state) = context
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target_key.state)
    else {
        return omega_checked_trees::expression::ExpressionHandle::invalid();
    };
    let statements = context
        .program
        .statement_table
        .statements(state.statement_nodes);
    let Some(statement) = statements.last() else {
        return omega_checked_trees::expression::ExpressionHandle::invalid();
    };
    match statement {
        StatementNode::Expression(expression) => *expression,
        StatementNode::Transition(transition)
            if !transition.continuation.is_valid()
                && matches!(transition.guard, TransitionGuardNode::Always) =>
        {
            match context
                .program
                .statement_table
                .transition_target(transition.target)
            {
                TransitionTargetNode::Value(expression) => *expression,
                _ => omega_checked_trees::expression::ExpressionHandle::invalid(),
            }
        }
        _ => omega_checked_trees::expression::ExpressionHandle::invalid(),
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
