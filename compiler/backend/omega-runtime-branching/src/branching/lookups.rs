use crate::RuntimeBranchingContext;
use omega_control_flow::{StateKey, StateParameterFlow};
use omega_platform_interface::HostCall;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::{StateCall, StateCallRole};

pub(super) fn host_call_for_statement<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    context
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn mutation_for_statement<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan omega_state_storage::StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

pub(super) fn state_call_for_operation<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    context
        .state_calls
        .statement_call(source_key, statement_index)
        .or_else(|| {
            context
                .state_calls
                .assignment_value_call(source_key, statement_index)
        })
}

pub(super) fn state_call_for_runtime_operation<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    operation: &RuntimeDispatchBodyOperation,
) -> Option<&'plan StateCall> {
    let (role, call_ordinal) = match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::InlineStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::StateCall {
            role, call_ordinal, ..
        } => (*role, Some(*call_ordinal)),
        _ => (StateCallRole::Statement, None),
    };

    match (role, call_ordinal) {
        (StateCallRole::AssignmentValue, Some(call_ordinal)) => context
            .state_calls
            .assignment_value_call_by_ordinal(
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            )
            .or_else(|| {
                context.state_calls.call_for_role(
                    operation.source_key,
                    operation.statement_index,
                    role,
                )
            }),
        (StateCallRole::CallArgument, Some(call_ordinal)) => context
            .state_calls
            .call_argument_call_by_ordinal(
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            )
            .or_else(|| {
                context.state_calls.call_for_role(
                    operation.source_key,
                    operation.statement_index,
                    role,
                )
            }),
        // A TransitionArgument call must ALSO resolve by ordinal: a multi-impl
        // `dyn` receiver call is monomorphized into one record PER IMPL under the
        // same (source, statement, role), distinguished only by call_ordinal --
        // a role-only lookup would expand the first impl for every candidate op.
        (StateCallRole::TransitionArgument, Some(call_ordinal)) => context
            .state_calls
            .transition_argument_call_by_ordinal(
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            )
            .or_else(|| {
                context.state_calls.call_for_role(
                    operation.source_key,
                    operation.statement_index,
                    role,
                )
            }),
        _ => {
            context
                .state_calls
                .call_for_role(operation.source_key, operation.statement_index, role)
        }
    }
}

pub(super) fn state_parameters<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    state_key: StateKey,
) -> &'plan [StateParameterFlow] {
    context
        .control_flow
        .state_by_key(state_key)
        .map(|state| context.control_flow.state_parameters(state))
        .unwrap_or(&[])
}

pub(super) fn state_statement_has_host_call(
    context: &RuntimeBranchingContext,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    context.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
}
