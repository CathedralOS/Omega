use super::entry::resolve_backend_entry_point;
use super::skeleton::{BackendPlanSkeletonInput, build_backend_plan_skeleton};
use super::timing::record_backend_phase;
use omega_control_flow_to_abstract_operations::{
    AbstractOperationLoweringInput, build_abstract_operation_plan,
};
use omega_abstract_operations_to_target_operations::build_target_operation_plan;
use omega_assigned_target_operations_to_machine_instructions::build_machine_instructions;
use omega_backend_plan::BackendPlan;
use omega_calling_conventions::build_host_abi_plan;
use omega_checked_trees::CheckedTrees;
use omega_control_flow::ControlFlowPlan;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_data_planning::build_target_data_plan;
use omega_layout::build_layout_plan;
use omega_machine_emission::{MachineEmissionInput, emit_machine_bytes};
use omega_object_file::object_entry_symbol_name;
use omega_object_file_planning::{ObjectPlanningInput, build_object_plan};
use omega_platform_interface::build_host_call_plan_with_workers;
use omega_relocations::{RelocationPlanningInput, build_relocation_plan};
use omega_runtime_abi::build_runtime_abi_plan;
use omega_runtime_bodies::{
    RuntimeDispatchBodyContext, build_runtime_dispatch_body_plan_with_workers,
};
use omega_runtime_branching::{RuntimeBranchingContext, build_runtime_branching_call_plan};
use omega_runtime_dispatch_loop::{
    RuntimeDispatchLoopContext, build_runtime_dispatch_loop_plan_with_workers,
};
use omega_runtime_storage::{
    RuntimeStorageContext, build_runtime_storage_plan_with_workers,
    runtime_frame_storage_alignment, runtime_frame_storage_size,
};
use omega_runtime_text::build_runtime_text_plan;
use omega_state_calls::{
    StateCallLowering, StateCallPlan, StateCallPlanningContext, StateCallRole,
    build_alias_flow_plan, build_state_call_plan_with_workers,
};
use omega_state_dispatch::{StateDispatchContext, build_state_dispatch_plan_with_workers};
use omega_state_graph::{
    RuntimeFlowPlan, RuntimeStateCallEdge, build_runtime_flow_plan,
    build_runtime_flow_plan_with_state_calls,
};
use omega_state_guards::build_state_guard_plan;
use omega_state_storage::{StateStoragePlanningContext, build_state_storage_plan_with_workers};
use omega_state_values::{StateValuePlanningContext, build_state_value_plan_with_workers};
use omega_target::NativeTarget;
use omega_target_operations_to_assigned_target_operations::build_assigned_target_operations;
use std::sync::Arc;

pub(super) fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<CheckedTrees>,
    target: NativeTarget,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    let entry_point = resolve_backend_entry_point(&program)?;
    let mut phase_timings = Arena::new();
    let host_abi = record_backend_phase(&mut phase_timings, "host abi", || {
        build_host_abi_plan(target)
    });
    let host_abi = Arc::new(host_abi);
    let host_call_program = Arc::clone(&program);
    let layout_program = Arc::clone(&program);
    let host_call_abi = Arc::clone(&host_abi);
    let host_call_workers = workers.clone();
    let (layouts, host_calls) =
        record_backend_phase(&mut phase_timings, "layout/host calls", || {
            workers.join2(
                move || build_layout_plan(&layout_program, target),
                move || {
                    build_host_call_plan_with_workers(
                        host_call_program,
                        target,
                        host_call_abi,
                        host_call_workers,
                    )
                },
            )
        });
    let layouts = layouts?;
    let host_calls = host_calls?;
    let host_calls = Arc::new(host_calls);
    let entry_key = control_flow
        .state_key_by_symbols(entry_point.machine_symbol, entry_point.state_symbol)
        .ok_or_else(|| Diagnostic::error("unknown runtime entry state"))?;
    let seed_runtime_flow = record_backend_phase(&mut phase_timings, "runtime flow/seed", || {
        build_runtime_flow_plan(&control_flow, entry_key)
    })?;
    let seed_runtime_flow = Arc::new(seed_runtime_flow);
    let state_calls = Arc::new(record_backend_phase(
        &mut phase_timings,
        "state calls/seed",
        || {
            build_state_call_plan_with_workers(
                Arc::new(StateCallPlanningContext {
                    control_flow: Arc::clone(&control_flow),
                    host_calls: Arc::clone(&host_calls),
                    runtime_flow: Arc::clone(&seed_runtime_flow),
                }),
                workers.clone(),
            )
        },
    ));
    let runtime_state_call_edges = dispatch_state_call_edges(state_calls.as_ref());
    let runtime_flow = if runtime_state_call_edges.is_empty() {
        seed_runtime_flow
    } else {
        Arc::new(record_backend_phase(
            &mut phase_timings,
            "runtime flow/state calls",
            || {
                build_runtime_flow_plan_with_state_calls(
                    &control_flow,
                    entry_key,
                    &runtime_state_call_edges,
                )
            },
        )?)
    };
    let state_calls = if runtime_state_call_edges.is_empty()
        || state_calls_cover_runtime_flow(state_calls.as_ref(), runtime_flow.as_ref())
    {
        state_calls
    } else {
        Arc::new(record_backend_phase(
            &mut phase_timings,
            "state calls",
            || {
                build_state_call_plan_with_workers(
                    Arc::new(StateCallPlanningContext {
                        control_flow: Arc::clone(&control_flow),
                        host_calls: Arc::clone(&host_calls),
                        runtime_flow: Arc::clone(&runtime_flow),
                    }),
                    workers.clone(),
                )
            },
        ))
    };
    let state_dispatch = record_backend_phase(&mut phase_timings, "state dispatch", || {
        build_state_dispatch_plan_with_workers(
            Arc::new(StateDispatchContext::from_runtime_flow(Arc::clone(
                &runtime_flow,
            ))),
            workers.clone(),
        )
    });
    let mut backend_plan = build_backend_plan_skeleton(BackendPlanSkeletonInput {
        target,
        host_abi: Arc::clone(&host_abi),
        host_calls: Arc::try_unwrap(host_calls).unwrap_or_else(|host_calls| (*host_calls).clone()),
        control_flow: Arc::clone(&control_flow),
        runtime_flow: Arc::clone(&runtime_flow),
        state_dispatch,
        state_guards: Default::default(),
        layouts,
        entry_key,
        phase_timings,
    });
    let mut phase_timings = std::mem::take(&mut backend_plan.phase_timings);
    backend_plan.state_calls = state_calls;
    backend_plan.alias_flow = record_backend_phase(&mut phase_timings, "alias flow", || {
        build_alias_flow_plan(backend_plan.state_calls.as_ref())
    });
    let state_storage_program = Arc::clone(&program);
    let state_values_program = Arc::clone(&program);
    let state_storage_context = Arc::new(StateStoragePlanningContext {
        control_flow: Arc::clone(&control_flow),
        runtime_flow: Arc::clone(&backend_plan.runtime_flow),
        state_calls: Arc::clone(&backend_plan.state_calls),
    });
    let state_values_context = Arc::new(StateValuePlanningContext {
        runtime_flow: Arc::clone(&backend_plan.runtime_flow),
        state_calls: Arc::clone(&backend_plan.state_calls),
    });
    let state_storage_workers = workers.clone();
    let state_values_workers = workers.clone();
    let (state_storage, state_values) =
        record_backend_phase(&mut phase_timings, "state storage/values", || {
            workers.join2(
                move || {
                    build_state_storage_plan_with_workers(
                        state_storage_program,
                        state_storage_context,
                        state_storage_workers,
                    )
                },
                move || {
                    build_state_value_plan_with_workers(
                        state_values_program,
                        state_values_context,
                        state_values_workers,
                    )
                },
            )
        });
    backend_plan.state_storage = Arc::new(state_storage);
    backend_plan.state_values = state_values;
    backend_plan.runtime_bodies = Arc::new(record_backend_phase(
        &mut phase_timings,
        "runtime bodies",
        || {
            build_runtime_dispatch_body_plan_with_workers(
                Arc::new(RuntimeDispatchBodyContext::new(
                    Arc::clone(&program),
                    Arc::clone(&control_flow),
                    Arc::clone(&backend_plan.host_calls),
                    Arc::clone(&backend_plan.state_dispatch),
                    Arc::clone(&backend_plan.state_calls),
                    Arc::clone(&backend_plan.state_storage),
                )),
                workers.clone(),
            )
        },
    ));
    let runtime_storage_context = Arc::new(RuntimeStorageContext::new(
        Arc::clone(&program),
        Arc::clone(&control_flow),
        Arc::clone(&backend_plan.layouts),
        Arc::clone(&backend_plan.runtime_bodies),
        Arc::clone(&backend_plan.state_calls),
        Arc::clone(&backend_plan.state_storage),
        backend_plan.target,
    ));
    let runtime_storage_workers = workers.clone();
    backend_plan.runtime_storage =
        record_backend_phase(&mut phase_timings, "runtime storage", || {
            build_runtime_storage_plan_with_workers(
                runtime_storage_context,
                runtime_storage_workers,
            )
        });
    backend_plan.state_guards = record_backend_phase(&mut phase_timings, "state guards", || {
        build_state_guard_plan(
            &program,
            backend_plan.state_dispatch.as_ref(),
            &backend_plan.control_flow,
            &backend_plan.layouts,
            &backend_plan.runtime_storage,
            backend_plan.entry_key.machine,
        )
    })
    .into();
    let runtime_loop_context = Arc::new(RuntimeDispatchLoopContext::from_parts(
        !backend_plan.state_dispatch.states.is_empty(),
        Arc::clone(&backend_plan.state_dispatch),
        backend_plan.entry_key,
        Arc::clone(&backend_plan.state_guards),
        Arc::clone(&backend_plan.runtime_bodies),
    ));
    let runtime_loop_workers = workers.clone();
    backend_plan.runtime_dispatch_loop =
        record_backend_phase(&mut phase_timings, "runtime loop", || {
            build_runtime_dispatch_loop_plan_with_workers(
                runtime_loop_context,
                runtime_loop_workers,
            )
        });
    backend_plan.runtime_branching_calls =
        record_backend_phase(&mut phase_timings, "runtime branching", || {
            build_runtime_branching_call_plan(&RuntimeBranchingContext {
                control_flow: &backend_plan.control_flow,
                host_calls: &backend_plan.host_calls,
                runtime_bodies: &backend_plan.runtime_bodies,
                state_calls: &backend_plan.state_calls,
                state_dispatch: &backend_plan.state_dispatch,
                state_guards: &backend_plan.state_guards,
                state_storage: &backend_plan.state_storage,
            })
        });
    backend_plan.runtime_text = record_backend_phase(&mut phase_timings, "runtime text", || {
        build_runtime_text_plan(&backend_plan.host_calls, &backend_plan.state_storage)
    });
    backend_plan.data = record_backend_phase(&mut phase_timings, "target data", || {
        build_target_data_plan(
            &backend_plan.host_calls,
            &backend_plan.state_storage,
            &backend_plan.state_values,
            &backend_plan.runtime_text,
        )
    });
    backend_plan.abstract_operations =
        record_backend_phase(&mut phase_timings, "abstract operations", || {
        let runtime_abi = build_runtime_abi_plan(backend_plan.target);
        build_abstract_operation_plan(&AbstractOperationLoweringInput {
            target: backend_plan.target,
            runtime_abi: &runtime_abi,
            entry_key: backend_plan.entry_key,
            entry_symbol: object_entry_symbol_name(&backend_plan.object).into(),
            program: program.as_ref(),
            host_abi: &backend_plan.host_abi,
            control_flow: &backend_plan.control_flow,
            host_calls: &backend_plan.host_calls,
            state_calls: &backend_plan.state_calls,
            alias_flow: &backend_plan.alias_flow,
            state_storage: &backend_plan.state_storage,
            runtime_flow: &backend_plan.runtime_flow,
            runtime_bodies: &backend_plan.runtime_bodies,
            runtime_branching_calls: &backend_plan.runtime_branching_calls,
            runtime_dispatch_loop: &backend_plan.runtime_dispatch_loop,
            runtime_storage: &backend_plan.runtime_storage,
            runtime_text: &backend_plan.runtime_text,
            state_guards: &backend_plan.state_guards,
            layouts: &backend_plan.layouts,
            data: &backend_plan.data,
        })
    });
    backend_plan.target_operations = record_backend_phase(&mut phase_timings, "target operations", || {
        build_target_operation_plan(backend_plan.target, &backend_plan.abstract_operations)
    });
    backend_plan.assigned_target_operations = record_backend_phase(
        &mut phase_timings,
        "assigned target operations",
        || build_assigned_target_operations(&backend_plan.target_operations),
    );
    backend_plan.machine_instructions =
        record_backend_phase(&mut phase_timings, "machine instructions", || {
            build_machine_instructions(&backend_plan.assigned_target_operations)
        })?;
    backend_plan.encoded_machine =
        record_backend_phase(&mut phase_timings, "machine emission", || {
            emit_machine_bytes(MachineEmissionInput {
                target: backend_plan.target,
                target_operations: &backend_plan.target_operations,
                assigned_target_operations: &backend_plan.assigned_target_operations,
                machine_instructions: &backend_plan.machine_instructions,
                host_abi: &backend_plan.host_abi,
                terminal_dispatch_index: backend_plan.runtime_dispatch_loop.terminal_dispatch_index,
            })
        })?;
    backend_plan.object = record_backend_phase(&mut phase_timings, "object plan", || {
        build_object_plan(ObjectPlanningInput {
            target: backend_plan.target,
            host_abi: &backend_plan.host_abi,
            layouts: &backend_plan.layouts,
            entry_machine_symbol: backend_plan.entry_key.machine,
            entry_machine_name: backend_plan.entry_machine_name(),
            entry_state_key: backend_plan.entry_key,
            encoded_machine: &backend_plan.encoded_machine,
            data: &backend_plan.data,
            runtime_frame_size: runtime_frame_storage_size(&backend_plan.runtime_storage),
            runtime_frame_alignment: runtime_frame_storage_alignment(&backend_plan.runtime_storage),
        })
    })?;
    backend_plan.relocations = record_backend_phase(&mut phase_timings, "relocations", || {
        build_relocation_plan(RelocationPlanningInput {
            target: backend_plan.target,
            instructions: &backend_plan.target_operations,
            encoded_machine: &backend_plan.encoded_machine,
            data: &backend_plan.data,
            object: &backend_plan.object,
            host_abi: &backend_plan.host_abi,
            entry_machine_name: backend_plan.entry_machine_name(),
        })
    })?;
    backend_plan.phase_timings = phase_timings;

    Ok(backend_plan)
}

fn dispatch_state_call_edges(state_calls: &StateCallPlan) -> Vec<RuntimeStateCallEdge> {
    state_calls
        .calls
        .iter()
        .filter_map(|(_, state_call)| {
            (state_call.required
                && state_call.role == StateCallRole::Statement
                && state_call.lowering == StateCallLowering::InlineBranching
                && state_call.argument_count == 0)
                .then_some(RuntimeStateCallEdge {
                    source_key: state_call.source_key,
                    statement_index: state_call.statement_index,
                    target_key: state_call.target_key,
                })
        })
        .collect()
}

fn state_calls_cover_runtime_flow(
    state_calls: &StateCallPlan,
    runtime_flow: &RuntimeFlowPlan,
) -> bool {
    runtime_flow
        .states
        .iter()
        .all(|(_, state)| state_calls.required_source_or_statement_target(state.key))
}
