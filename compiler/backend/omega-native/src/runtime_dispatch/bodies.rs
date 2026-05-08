mod model;

use crate::control_flow::OperationKind;
use crate::control_flow::StateKey;
use crate::control_flow::{ControlFlowPlan, Operation};
use crate::host_calls::{HostCall, HostCallPlan};
use crate::plan::NativePlan;
use crate::runtime_dispatch::states::DispatchState;
use crate::state_calls::{StateCall, StateCallLowering};
use crate::state_storage::{StateLocalStorage, StateMutation, StateStoragePlan};
pub use model::{
    RuntimeDispatchBody, RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
    RuntimeDispatchBodyPlan,
};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::name::ProgramName;
use std::sync::Arc;

pub fn build_runtime_dispatch_body_plan(native_plan: &NativePlan) -> RuntimeDispatchBodyPlan {
    let workers = WorkerPool::with_available_parallelism();

    build_runtime_dispatch_body_plan_with_workers(
        Arc::new(RuntimeDispatchBodyContext::from_native_plan(native_plan)),
        native_plan
            .state_dispatch
            .states
            .iter()
            .map(|(_, dispatch_state)| dispatch_state.clone())
            .collect(),
        workers.handle(),
    )
}

pub fn build_runtime_dispatch_body_plan_with_workers(
    context: Arc<RuntimeDispatchBodyContext>,
    dispatch_states: Vec<DispatchState>,
    workers: WorkerPoolHandle,
) -> RuntimeDispatchBodyPlan {
    if dispatch_states.is_empty() {
        return RuntimeDispatchBodyPlan::default();
    }

    let dispatch_states = Arc::new(dispatch_states);
    let state_count = dispatch_states.len();
    let context_for_bodies = Arc::clone(&context);
    let collected_bodies = workers.map_ordered(state_count, move |index| {
        let dispatch_state = dispatch_states
            .get(index)
            .expect("runtime-body worker index should be in range");

        build_dispatch_body(&context_for_bodies, dispatch_state)
    });

    let mut plan = RuntimeDispatchBodyPlan::default();

    for collected_body in collected_bodies {
        let operations = plan.operations.insert_many(collected_body.operations);

        plan.bodies.insert(RuntimeDispatchBody {
            key: collected_body.key,
            machine: collected_body.machine,
            state: collected_body.state,
            dispatch_index: collected_body.dispatch_index,
            operations,
        });
    }

    plan
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyContext {
    pub control_flow: ControlFlowPlan,
    pub host_calls: HostCallPlan,
    pub state_calls: crate::state_calls::StateCallPlan,
    pub state_storage: StateStoragePlan,
}

impl RuntimeDispatchBodyContext {
    pub fn from_native_plan(native_plan: &NativePlan) -> Self {
        Self {
            control_flow: native_plan.control_flow.clone(),
            host_calls: native_plan.host_calls.clone(),
            state_calls: native_plan.state_calls.clone(),
            state_storage: native_plan.state_storage.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CollectedRuntimeDispatchBody {
    key: StateKey,
    machine: ProgramName,
    state: ProgramName,
    dispatch_index: u32,
    operations: Vec<RuntimeDispatchBodyOperation>,
}

fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let mut operations = Vec::new();
    append_state_body_operations(
        context,
        dispatch_state.key,
        &dispatch_state.machine,
        &dispatch_state.state,
        &mut operations,
        &mut Vec::new(),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        machine: dispatch_state.machine.clone(),
        state: dispatch_state.state.clone(),
        dispatch_index: dispatch_state.dispatch_index,
        operations,
    }
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    machine_name: &ProgramName,
    state_name: &ProgramName,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
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
                machine_name,
                state_name,
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
            append_state_call_body_operation(context, state_call, operations, visiting);
            continue;
        }

        if let Some(local_storage) =
            local_storage_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                machine_name,
                state_name,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::LocalStorage {
                    name: local_storage.name.clone(),
                    type_name: local_storage.type_name.clone(),
                },
            ));
            continue;
        }

        if let Some(mutation) =
            mutation_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                machine_name,
                state_name,
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
                machine_name,
                state_name,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Other,
            ));
        }
    }

    visiting.pop();
}

fn append_state_call_body_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    visiting: &mut Vec<StateKey>,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.push(body_operation(
            state_call.source_key,
            &state_call.source_machine,
            &state_call.source_state,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                target_key: state_call.target_key,
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
                argument_count: state_call.argument_count,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            &state_call.target_machine,
            &state_call.target_state,
            operations,
            visiting,
        );
        return;
    }

    if state_has_no_transitions(context, state_call.target_key) {
        operations.push(body_operation(
            state_call.source_key,
            &state_call.source_machine,
            &state_call.source_state,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineStateCall {
                target_key: state_call.target_key,
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
                argument_count: state_call.argument_count,
                lowering: state_call.lowering,
            },
        ));
        append_state_body_operations(
            context,
            state_call.target_key,
            &state_call.target_machine,
            &state_call.target_state,
            operations,
            visiting,
        );
        return;
    }

    operations.push(body_operation(
        state_call.source_key,
        &state_call.source_machine,
        &state_call.source_state,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCall {
            target_key: state_call.target_key,
            target_machine: state_call.target_machine.clone(),
            target_state: state_call.target_state.clone(),
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    ));
}

fn body_operation(
    source_key: StateKey,
    machine_name: &ProgramName,
    state_name: &ProgramName,
    statement_index: usize,
    kind: RuntimeDispatchBodyOperationKind,
) -> RuntimeDispatchBodyOperation {
    RuntimeDispatchBodyOperation {
        source_key,
        source_machine: machine_name.clone(),
        source_state: state_name.clone(),
        statement_index,
        kind,
    }
}

fn state_operations<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> Option<&'plan [Operation]> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.key == state_key))
        .and_then(|state| context.control_flow.operations.span(state.operations))
}

fn state_has_no_transitions(context: &RuntimeDispatchBodyContext, state_key: StateKey) -> bool {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.key == state_key))
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_none_or(|transitions| transitions.is_empty())
}

fn host_call_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    context
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == state_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn state_call_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    context
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_key == state_key && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn local_storage_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateLocalStorage> {
    context
        .state_storage
        .locals
        .iter()
        .find(|(_, local)| {
            local.source_key == state_key && local.statement_index == statement_index
        })
        .map(|(_, local)| local)
}

fn mutation_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == state_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
