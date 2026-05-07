use crate::control_flow::OperationKind;
use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use crate::state_calls::{StateCall, StateCallLowering};
use crate::state_storage::{StateMutationKind, StateMutationLowering};
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeDispatchBodyPlan {
    pub bodies: Arena<RuntimeDispatchBody>,
    pub operations: Arena<RuntimeDispatchBodyOperation>,
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
    let mut plan = RuntimeDispatchBodyPlan::default();

    for (_, dispatch_state) in native_plan.state_dispatch.states.iter() {
        let mut operations = Vec::new();
        append_state_body_operations(
            native_plan,
            &dispatch_state.machine,
            &dispatch_state.state,
            &mut operations,
            &mut Vec::new(),
        );
        let operations = plan.operations.insert_many(operations);

        plan.bodies.insert(RuntimeDispatchBody {
            machine: dispatch_state.machine.clone(),
            state: dispatch_state.state.clone(),
            dispatch_index: dispatch_state.dispatch_index,
            operations,
        });
    }

    plan
}

fn append_state_body_operations(
    native_plan: &NativePlan,
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

    let Some(state_operations) = state_operations(native_plan, machine_name, state_name) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        if let Some(host_call) = host_call_for_statement(
            native_plan,
            machine_name,
            state_name,
            operation.statement_index,
        ) {
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

        if let Some(state_call) = state_call_for_statement(
            native_plan,
            machine_name,
            state_name,
            operation.statement_index,
        ) {
            append_state_call_body_operation(native_plan, state_call, operations, visiting);
            continue;
        }

        if let Some(local_storage) = local_storage_for_statement(
            native_plan,
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

        if let Some(mutation) = mutation_for_statement(
            native_plan,
            machine_name,
            state_name,
            operation.statement_index,
        ) {
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
    native_plan: &NativePlan,
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
            native_plan,
            &state_call.target_machine,
            &state_call.target_state,
            operations,
            visiting,
        );
        return;
    }

    if state_has_no_transitions(
        native_plan,
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
            native_plan,
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
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<&'plan [crate::control_flow::Operation]> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .and_then(|state| native_plan.control_flow.operations.span(state.operations))
}

fn state_has_no_transitions(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> bool {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.name == machine_name)
        .and_then(|(_, machine)| native_plan.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.name == state_name))
        .and_then(|state| native_plan.control_flow.transitions.span(state.transitions))
        .is_none_or(|transitions| transitions.is_empty())
}

fn host_call_for_statement<'plan>(
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    native_plan
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
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    native_plan
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
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan crate::state_storage::StateLocalStorage> {
    native_plan
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
    native_plan: &'plan NativePlan,
    machine_name: &str,
    state_name: &str,
    statement_index: usize,
) -> Option<&'plan crate::state_storage::StateMutation> {
    native_plan
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
