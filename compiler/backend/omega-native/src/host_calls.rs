use crate::abi::HostAbiPlan;
use crate::control_flow::{ControlFlowPlan, StateKey};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_target::NativeTarget;
use omega_typed_program::Program;
use omega_typed_program::machine::Machine;
use omega_typed_program::state::State;
use omega_typed_program::statement::{Call, Statement};
use std::sync::Arc;

mod lowering;
mod model;
mod static_values;

use lowering::{
    find_platform_call_lowering, host_operation, lower_host_call_arguments, platform_call_name,
    platform_call_receiver_type,
};
pub use model::{
    HostCall, HostCallArgument, HostCallArgumentKind, HostCallPlan, LoweredHostOperation,
    UnsupportedHostCall,
};
use static_values::{
    StaticValue, apply_call_static_effects, apply_static_assignment, initial_static_values,
};

pub fn build_host_call_plan(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
) -> Result<HostCallPlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_host_call_plan_with_workers(
        Arc::new(program.clone()),
        target,
        Arc::new(host_abi.clone()),
        workers.handle(),
    )
}

pub fn build_host_call_plan_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    host_abi: Arc<HostAbiPlan>,
    workers: WorkerPoolHandle,
) -> Result<HostCallPlan, Diagnostic> {
    if program.machines.is_empty() {
        return Ok(HostCallPlan::default());
    }

    let machine_count = program.machines.len();
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines
            .get(index)
            .expect("host-call worker index should be in range");
        let mut machine_plan = HostCallPlan::default();

        collect_machine_host_calls(&program, target, &host_abi, machine, &mut machine_plan)
            .map(|_| machine_plan)
    });

    let mut plan = HostCallPlan::default();

    for machine_plan in machine_plans {
        merge_host_call_plan(&mut plan, machine_plan?);
    }

    Ok(plan)
}

fn merge_host_call_plan(target: &mut HostCallPlan, source: HostCallPlan) {
    for (_, unsupported_call) in source.unsupported_calls.iter() {
        target.unsupported_calls.insert(unsupported_call.clone());
    }

    for (_, call) in source.calls.iter() {
        let operations = target.operations.insert_many(
            source
                .operations
                .span_or_empty(call.operations)
                .iter()
                .cloned(),
        );
        let arguments = target.arguments.insert_many(
            source
                .arguments
                .span_or_empty(call.arguments)
                .iter()
                .cloned(),
        );

        target.calls.insert(HostCall {
            operations,
            arguments,
            ..call.clone()
        });
    }
}

fn collect_machine_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    for state in &machine.states {
        collect_state_host_calls(program, target, host_abi, machine, state, plan)?;
    }

    Ok(())
}

fn collect_state_host_calls(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let mut static_values = initial_static_values(machine);

    for (statement_index, statement) in state.statements.iter().enumerate() {
        match statement {
            Statement::Assignment(assignment) => {
                apply_static_assignment(&mut static_values, &assignment.target, &assignment.value);
                continue;
            }
            Statement::Call(call) => {
                collect_call_host_lowering(
                    program,
                    target,
                    host_abi,
                    machine,
                    state,
                    statement_index,
                    call,
                    &static_values,
                    plan,
                )?;
                apply_call_static_effects(&mut static_values, call);
            }
            _ => {}
        }
    }

    Ok(())
}

fn collect_call_host_lowering(
    program: &Program,
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    call: &Call,
    static_values: &[(String, StaticValue)],
    plan: &mut HostCallPlan,
) -> Result<(), Diagnostic> {
    let Some(platform_name) = platform_call_receiver_type(program, machine, call) else {
        return Ok(());
    };

    let Some(lowering) = find_platform_call_lowering(host_abi, &platform_name, call) else {
        let platform_call = platform_call_name(call);
        plan.unsupported_calls.insert(UnsupportedHostCall {
            source_key: StateKey::default(),
            machine: machine.name.clone(),
            state: state.name.clone(),
            statement_index,
            platform_call: platform_call.clone(),
            reason: format!("no native lowering for target {target:?}"),
        });
        return Ok(());
    };

    let operations = host_abi
        .host_operations
        .span(lowering.operations)
        .map(|operations| {
            plan.operations.insert_many(
                operations
                    .iter()
                    .map(|operation| host_operation(&operation.capability, &operation.operation)),
            )
        })
        .unwrap_or_else(HandleSpan::empty);
    let arguments = plan
        .arguments
        .insert_many(lower_host_call_arguments(call, static_values));
    plan.calls.insert(HostCall {
        source_key: StateKey::default(),
        machine: machine.name.clone(),
        state: state.name.clone(),
        statement_index,
        platform_call: platform_call_name(call),
        data: lowering.data,
        operations,
        arguments,
    });
    Ok(())
}

pub fn attach_host_call_state_keys(plan: &mut HostCallPlan, control_flow: &ControlFlowPlan) {
    plan.calls.for_each_mut(|_, call| {
        call.source_key = control_flow
            .state_key_by_names(&call.machine, &call.state)
            .unwrap_or_default();
    });

    plan.unsupported_calls.for_each_mut(|_, call| {
        call.source_key = control_flow
            .state_key_by_names(&call.machine, &call.state)
            .unwrap_or_default();
    });
}
