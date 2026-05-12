use super::entry::resolve_backend_entry_point;
use super::skeleton::{BackendPlanSkeletonInput, build_backend_plan_skeleton};
use super::timing::record_backend_phase;
use omega_backend_plan::BackendPlan;
use omega_calling_conventions::build_host_abi_plan;
use omega_control_flow::ControlFlowPlan;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_data_planning::build_target_data_plan;
use omega_instruction_selection::{InstructionSelectionInput, build_instruction_plan};
use omega_layout::build_layout_plan;
use omega_machine_emission::{MachineEmissionInput, emit_machine_bytes};
use omega_object_planning::{ObjectPlanningInput, build_object_plan};
use omega_platform_interface::build_host_call_plan_with_workers;
use omega_relocations::{RelocationPlanningInput, build_relocation_plan};
use omega_runtime_bodies::{
    RuntimeDispatchBodyContext, build_runtime_dispatch_body_plan_with_workers,
};
use omega_runtime_branching::{RuntimeBranchingContext, build_runtime_branching_call_plan};
use omega_runtime_dispatch_loop::{
    RuntimeDispatchLoopContext, build_runtime_dispatch_loop_plan_with_workers,
    runtime_dispatch_loop_inputs,
};
use omega_runtime_storage::{
    RuntimeStorageContext, build_runtime_storage_plan_with_workers,
    runtime_frame_storage_alignment, runtime_frame_storage_size, runtime_storage_body_inputs,
};
use omega_runtime_text::build_runtime_text_plan;
use omega_state_calls::{
    StateCallPlanningContext, build_alias_flow_plan, build_state_call_plan_with_workers,
};
use omega_state_dispatch::{
    StateDispatchContext, build_state_dispatch_plan_with_workers, runtime_state_inputs,
};
use omega_state_graph::build_runtime_flow_plan;
use omega_state_guards::build_state_guard_plan;
use omega_state_storage::{StateStoragePlanningContext, build_state_storage_plan_with_workers};
use omega_state_values::{StateValuePlanningContext, build_state_value_plan_with_workers};
use omega_target::NativeTarget;
use omega_target_operations_to_machine_program::build_machine_program;
use omega_checked_trees::Program;
use std::sync::Arc;

pub(super) fn build_backend_plan_from_control_flow_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    control_flow: Arc<ControlFlowPlan>,
    workers: WorkerPoolHandle,
) -> Result<BackendPlan, Diagnostic> {
    let entry_point = resolve_backend_entry_point(&program)?;
    let mut phase_timings = Vec::new();
    let host_abi = record_backend_phase(&mut phase_timings, "host abi", || {
        build_host_abi_plan(target)
    });
    let host_call_program = Arc::clone(&program);
    let layout_program = Arc::clone(&program);
    let host_call_abi = Arc::new(host_abi.clone());
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
    let entry_key = control_flow
        .state_key_by_symbols(entry_point.machine_symbol, entry_point.state_symbol)
        .ok_or_else(|| Diagnostic::error("unknown runtime state `main.entry`"))?;
    let runtime_flow = record_backend_phase(&mut phase_timings, "runtime flow", || {
        build_runtime_flow_plan(&control_flow, entry_key)
    })?;
    let state_dispatch = record_backend_phase(&mut phase_timings, "state dispatch", || {
        build_state_dispatch_plan_with_workers(
            Arc::new(StateDispatchContext::from_runtime_flow(&runtime_flow)),
            runtime_state_inputs(&runtime_flow),
            workers.clone(),
        )
    });
    let mut backend_plan = build_backend_plan_skeleton(BackendPlanSkeletonInput {
        target,
        host_abi,
        host_calls,
        control_flow: (*control_flow).clone(),
        runtime_flow,
        state_dispatch,
        state_guards: Default::default(),
        layouts,
        entry_key,
        phase_timings,
    });
    let mut phase_timings = std::mem::take(&mut backend_plan.phase_timings);
    backend_plan.state_calls = record_backend_phase(&mut phase_timings, "state calls", || {
        build_state_call_plan_with_workers(
            Arc::new(StateCallPlanningContext {
                control_flow: backend_plan.control_flow.clone(),
                host_calls: backend_plan.host_calls.clone(),
                runtime_flow: backend_plan.runtime_flow.clone(),
            }),
            workers.clone(),
        )
    });
    backend_plan.alias_flow = record_backend_phase(&mut phase_timings, "alias flow", || {
        build_alias_flow_plan(&backend_plan.state_calls)
    });
    let state_storage_program = Arc::clone(&program);
    let state_values_program = Arc::clone(&program);
    let state_storage_context = Arc::new(StateStoragePlanningContext {
        control_flow: backend_plan.control_flow.clone(),
        runtime_flow: backend_plan.runtime_flow.clone(),
        state_calls: backend_plan.state_calls.clone(),
    });
    let state_values_context = Arc::new(StateValuePlanningContext {
        runtime_flow: backend_plan.runtime_flow.clone(),
        state_calls: backend_plan.state_calls.clone(),
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
    backend_plan.state_storage = state_storage;
    backend_plan.state_values = state_values;
    backend_plan.runtime_bodies =
        record_backend_phase(&mut phase_timings, "runtime bodies", || {
            build_runtime_dispatch_body_plan_with_workers(
                Arc::new(RuntimeDispatchBodyContext::new(
                    &backend_plan.control_flow,
                    &backend_plan.host_calls,
                    &backend_plan.state_calls,
                    &backend_plan.state_storage,
                )),
                backend_plan
                    .state_dispatch
                    .states
                    .iter()
                    .map(|(_, dispatch_state)| dispatch_state.clone())
                    .collect(),
                workers.clone(),
            )
        });
    let runtime_storage_context = Arc::new(RuntimeStorageContext::new(
        &backend_plan.control_flow,
        &backend_plan.layouts,
        &backend_plan.runtime_bodies,
        &backend_plan.state_storage,
        backend_plan.target,
    ));
    let runtime_storage_inputs = runtime_storage_body_inputs(&backend_plan.runtime_bodies);
    let runtime_storage_workers = workers.clone();
    backend_plan.runtime_storage =
        record_backend_phase(&mut phase_timings, "runtime storage", || {
            build_runtime_storage_plan_with_workers(
                runtime_storage_context,
                runtime_storage_inputs,
                runtime_storage_workers,
            )
        });
    backend_plan.state_guards = record_backend_phase(&mut phase_timings, "state guards", || {
        build_state_guard_plan(
            &program,
            &backend_plan.state_dispatch,
            &backend_plan.control_flow,
            &backend_plan.layouts,
            &backend_plan.runtime_storage,
            backend_plan.entry_key.machine,
        )
    });
    let runtime_loop_context = Arc::new(RuntimeDispatchLoopContext::from_parts(
        !backend_plan.state_dispatch.states.is_empty(),
        &backend_plan.state_dispatch,
        backend_plan.entry_key,
        backend_plan.state_guards.clone(),
        backend_plan.runtime_bodies.clone(),
    ));
    let runtime_loop_inputs = runtime_dispatch_loop_inputs(&backend_plan.state_dispatch);
    let runtime_loop_workers = workers.clone();
    backend_plan.runtime_dispatch_loop =
        record_backend_phase(&mut phase_timings, "runtime loop", || {
            build_runtime_dispatch_loop_plan_with_workers(
                runtime_loop_context,
                runtime_loop_inputs,
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
    backend_plan.instructions = record_backend_phase(&mut phase_timings, "instructions", || {
        build_instruction_plan(&InstructionSelectionInput {
            target: backend_plan.target,
            entry_key: backend_plan.entry_key,
            entry_symbol: backend_plan.object.entry_symbol.clone(),
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
    backend_plan.machine_program =
        record_backend_phase(&mut phase_timings, "machine program", || {
            build_machine_program(&backend_plan.instructions)
        })?;
    backend_plan.encoded_machine =
        record_backend_phase(&mut phase_timings, "machine emission", || {
            emit_machine_bytes(MachineEmissionInput {
                target: backend_plan.target,
                instructions: &backend_plan.instructions,
                machine_program: &backend_plan.machine_program,
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
            instructions: &backend_plan.instructions,
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
