use omega_calling_conventions::HostAbiPlan;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_target::NativeTarget;
use omega_typed_program::Program;
use std::sync::Arc;

mod collection;
mod lowering;
mod static_values;

use collection::collect_machine_host_calls;
use omega_platform_interface::{
    HostCall, HostCallArgument, HostCallArgumentKind, HostCallPlan, LoweredHostOperation,
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
