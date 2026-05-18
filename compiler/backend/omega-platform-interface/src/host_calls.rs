use omega_calling_conventions::HostAbiPlan;
use omega_checked_trees::Program;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_target::NativeTarget;
use std::sync::Arc;

mod collection;
mod lowering;
mod static_values;

use crate::{HostCall, HostCallArgument, HostCallArgumentKind, HostCallPlan};
use collection::collect_machine_host_calls;
use omega_checked_trees::machine::Machine;
use omega_checked_trees::statement::StatementNode;

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
    if program.machines().is_empty() {
        return Ok(HostCallPlan::default());
    }

    let machine_count = program.machines().len();
    let plan_capacity = host_call_plan_capacity(&program, &host_abi);
    let machine_plans = workers.map_ordered(machine_count, move |index| {
        let machine = program
            .machines()
            .get(index)
            .expect("host-call worker index should be in range");
        let capacity = machine_host_call_plan_capacity(&program, &host_abi, machine);
        let mut machine_plan = HostCallPlan::with_capacity(
            capacity.call_count,
            capacity.call_count,
            capacity.operation_count,
            capacity.argument_count,
        );

        collect_machine_host_calls(&program, target, &host_abi, machine, &mut machine_plan)
            .map(|_| machine_plan)
    });

    let mut plan = HostCallPlan::with_capacity(
        plan_capacity.call_count,
        plan_capacity.call_count,
        plan_capacity.operation_count,
        plan_capacity.argument_count,
    );

    for machine_plan in machine_plans {
        merge_host_call_plan(&mut plan, machine_plan?);
    }

    Ok(plan)
}

#[derive(Debug, Clone, Copy, Default)]
struct HostCallPlanCapacity {
    call_count: usize,
    operation_count: usize,
    argument_count: usize,
}

fn host_call_plan_capacity(program: &Program, host_abi: &HostAbiPlan) -> HostCallPlanCapacity {
    program
        .machines()
        .iter()
        .map(|machine| machine_host_call_plan_capacity(program, host_abi, machine))
        .fold(HostCallPlanCapacity::default(), |total, next| {
            HostCallPlanCapacity {
                call_count: total.call_count.saturating_add(next.call_count),
                operation_count: total.operation_count.saturating_add(next.operation_count),
                argument_count: total.argument_count.saturating_add(next.argument_count),
            }
        })
}

fn machine_host_call_plan_capacity(
    program: &Program,
    host_abi: &HostAbiPlan,
    machine: &Machine,
) -> HostCallPlanCapacity {
    let max_lowering_operations = host_abi
        .platform_call_lowerings
        .iter()
        .map(|(_, lowering)| usize::try_from(lowering.operations.count()).unwrap_or(usize::MAX))
        .max()
        .unwrap_or(0);
    let mut capacity = HostCallPlanCapacity::default();

    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Call(call) = statement else {
                continue;
            };

            capacity.call_count = capacity.call_count.saturating_add(1);
            capacity.operation_count = capacity
                .operation_count
                .saturating_add(max_lowering_operations);
            capacity.argument_count = capacity.argument_count.saturating_add(
                program
                    .statement_table
                    .expression_handles(call.arguments)
                    .len(),
            );
        }
    }

    capacity
}

fn merge_host_call_plan(target: &mut HostCallPlan, source: HostCallPlan) {
    target
        .unsupported_calls
        .reserve(source.unsupported_calls.len());
    target.calls.reserve(source.calls.len());
    target.operations.reserve(source.operations.len());
    target.arguments.reserve(source.arguments.len());

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
                .map(|argument| HostCallArgument {
                    kind: match &argument.kind {
                        HostCallArgumentKind::Text(value) => {
                            HostCallArgumentKind::Text(value.clone())
                        }
                        HostCallArgumentKind::Integer(value) => {
                            HostCallArgumentKind::Integer(*value)
                        }
                        HostCallArgumentKind::Expression(expression) => {
                            HostCallArgumentKind::Expression(
                                target
                                    .expressions
                                    .copy_from(&source.expressions, *expression),
                            )
                        }
                    },
                }),
        );

        target.calls.insert(HostCall {
            operations,
            arguments,
            ..call.clone()
        });
    }
}
