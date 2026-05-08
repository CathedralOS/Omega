use crate::control_flow::OperationKind;
use crate::control_flow::{ControlFlowPlan, Operation};
use crate::host_calls::{HostCall, HostCallPlan};
use crate::plan::NativePlan;
use crate::runtime_dispatch::states::DispatchState;
use crate::state_calls::{StateCall, StateCallLowering};
use crate::state_storage::{
    StateLocalStorage, StateMutation, StateMutationKind, StateMutationLowering, StateStoragePlan,
};
use omega_core::arena::{Arena, HandleSpan, PagedArena};
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyPlan {
    pub bodies: Arena<RuntimeDispatchBody>,
    pub operations: PagedArena<RuntimeDispatchBodyOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBody {
    pub machine: String,
    pub state: String,
    pub dispatch_index: u32,
    pub operations: HandleSpan<RuntimeDispatchBodyOperation>,
}

impl Default for RuntimeDispatchBody {
    fn default() -> Self {
        Self {
            machine: String::new(),
            state: String::new(),
            dispatch_index: 0,
            operations: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDispatchBodyOperation {
    pub source_machine: String,
    pub source_state: String,
    pub statement_index: usize,
    pub kind: RuntimeDispatchBodyOperationKind,
}

impl Default for RuntimeDispatchBodyOperation {
    fn default() -> Self {
        Self {
            source_machine: String::new(),
            source_state: String::new(),
            statement_index: 0,
            kind: RuntimeDispatchBodyOperationKind::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RuntimeDispatchBodyOperationKind {
    HostCall {
        platform_call: String,
    },
    InlineLeafStateCall {
        target_machine: String,
        target_state: String,
        argument_count: usize,
    },
    InlineStateCall {
        target_machine: String,
        target_state: String,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    StateCall {
        target_machine: String,
        target_state: String,
        argument_count: usize,
        lowering: StateCallLowering,
    },
    LocalStorage {
        name: String,
        type_name: String,
    },
    Mutation {
        mutation_kind: StateMutationKind,
        lowering: StateMutationLowering,
    },
    #[default]
    Other,
}

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
    machine: String,
    state: String,
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
        &dispatch_state.machine,
        &dispatch_state.state,
        &mut operations,
        &mut Vec::new(),
    );

    CollectedRuntimeDispatchBody {
        machine: dispatch_state.machine.clone(),
        state: dispatch_state.state.clone(),
        dispatch_index: dispatch_state.dispatch_index,
        operations,
    }
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    visiting: &mut Vec<(String, String)>,
) {
    if visiting
        .iter()
        .any(|(machine, state)| machine == machine_name && state == state_name)
    {
        return;
    }
    visiting.push((machine_name.to_owned(), state_name.to_owned()));

    let Some(state_operations) = state_operations(context, machine_name, state_name) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        if let Some(host_call) =
            host_call_for_statement(context, machine_name, state_name, operation.statement_index)
        {
            operations.push(body_operation(
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
            state_call_for_statement(context, machine_name, state_name, operation.statement_index)
        {
            append_state_call_body_operation(context, state_call, operations, visiting);
            continue;
        }

        if let Some(local_storage) = local_storage_for_statement(
            context,
            machine_name,
            state_name,
            operation.statement_index,
        ) {
            operations.push(body_operation(
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
            mutation_for_statement(context, machine_name, state_name, operation.statement_index)
        {
            operations.push(body_operation(
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
    visiting: &mut Vec<(String, String)>,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.push(body_operation(
            &state_call.source_machine,
            &state_call.source_state,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
                argument_count: state_call.argument_count,
            },
        ));
        append_state_body_operations(
            context,
            &state_call.target_machine,
            &state_call.target_state,
            operations,
            visiting,
        );
        return;
    }

    if state_has_no_transitions(
        context,
        &state_call.target_machine,
        &state_call.target_state,
    ) {
        operations.push(body_operation(
            &state_call.source_machine,
            &state_call.source_state,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineStateCall {
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
                argument_count: state_call.argument_count,
                lowering: state_call.lowering,
            },
        ));
        append_state_body_operations(
            context,
            &state_call.target_machine,
            &state_call.target_state,
            operations,
            visiting,
        );
        return;
    }

    operations.push(body_operation(
        &state_call.source_machine,
        &state_call.source_state,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCall {
            target_machine: state_call.target_machine.clone(),
            target_state: state_call.target_state.clone(),
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    ));
}

fn body_operation(
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
    kind: RuntimeDispatchBodyOperationKind,
) -> RuntimeDispatchBodyOperation {
    RuntimeDispatchBodyOperation {
        source_machine: machine_name.to_owned(),
        source_state: state_name.to_owned(),
        statement_index,
        kind,
    }
}

fn state_operations<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
) -> Option<&'plan [Operation]> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .and_then(|state| context.control_flow.operations.span(state.operations))
}

fn state_has_no_transitions(
    context: &RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
) -> bool {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_none_or(|transitions| transitions.is_empty())
}

fn host_call_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    context
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.machine == machine_name
                && host_call.state == state_name
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

fn state_call_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    context
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_machine == machine_name
                && state_call.source_state == state_name
                && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

fn local_storage_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan StateLocalStorage> {
    context
        .state_storage
        .locals
        .iter()
        .find(|(_, local)| {
            local.machine == machine_name
                && local.state == state_name
                && local.statement_index == statement_index
        })
        .map(|(_, local)| local)
}

fn mutation_for_statement<'plan>(
    context: &'plan RuntimeDispatchBodyContext,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.machine == machine_name
                && mutation.state == state_name
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
