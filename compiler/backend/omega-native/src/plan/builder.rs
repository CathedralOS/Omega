use super::NativePlan;
use super::entry::{ENTRY_MACHINE_NAME, resolve_native_entry_point};
use super::skeleton::{NativePlanSkeletonInput, build_native_plan_skeleton};
use super::timing::record_native_phase;
use crate::abi::build_host_abi_plan;
use crate::alias_flow::build_alias_flow_plan;
use crate::control_flow::build_control_flow_plan_with_workers;
use crate::data::build_native_data_plan;
use crate::host_calls::{attach_host_call_state_keys, build_host_call_plan_with_workers};
use crate::instructions::build_instruction_plan;
use crate::machine_code::build_machine_code_plan;
use crate::object::build_object_plan;
use crate::relocations::build_relocation_plan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBodyContext, build_runtime_dispatch_body_plan_with_workers,
};
use crate::runtime_dispatch::branching::build_runtime_branching_call_plan;
use crate::runtime_dispatch::loop_plan::{
    RuntimeDispatchLoopContext, build_runtime_dispatch_loop_plan_with_workers,
    runtime_dispatch_loop_inputs,
};
use crate::runtime_flow::build_runtime_flow_plan;
use crate::runtime_storage::{
    RuntimeStorageContext, build_runtime_storage_plan_with_workers, runtime_storage_body_inputs,
};
use crate::runtime_text::build_runtime_text_plan;
use crate::state_analysis::StateAnalysisContext;
use crate::state_calls::build_state_call_plan_with_workers;
use crate::state_dispatch::{
    StateDispatchContext, build_state_dispatch_plan_with_workers, runtime_state_inputs,
};
use crate::state_guards::build_state_guard_plan;
use crate::state_storage::build_state_storage_plan_with_workers;
use crate::state_values::build_state_value_plan_with_workers;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::WorkerPoolHandle;
use omega_layout::build_layout_plan;
use omega_target::NativeTarget;
use omega_typed_program::Program;
use std::sync::Arc;

pub(super) fn build_native_plan_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    workers: WorkerPoolHandle,
) -> Result<NativePlan, Diagnostic> {
    let entry_point = resolve_native_entry_point(&program)?;
    let mut phase_timings = Vec::new();
    let host_abi = record_native_phase(&mut phase_timings, "host abi", || {
        build_host_abi_plan(target)
    });
    let host_call_program = Arc::clone(&program);
    let layout_program = Arc::clone(&program);
    let control_flow_program = Arc::clone(&program);
    let host_call_abi = Arc::new(host_abi.clone());
    let control_flow_workers = workers.clone();
    let host_call_workers = workers.clone();
    let (control_flow, layouts, host_calls) =
        record_native_phase(&mut phase_timings, "control/layout/host calls", || {
            workers.join3(
                move || {
                    build_control_flow_plan_with_workers(control_flow_program, control_flow_workers)
                },
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
    let control_flow = control_flow?;
    let layouts = layouts?;
    let mut host_calls = host_calls?;
    attach_host_call_state_keys(&mut host_calls, &control_flow);
    let entry_key = control_flow
        .state_key_by_symbols(entry_point.machine_symbol, entry_point.state_symbol)
        .ok_or_else(|| Diagnostic::error("unknown runtime state `main.entry`"))?;
    let runtime_flow = record_native_phase(&mut phase_timings, "runtime flow", || {
        build_runtime_flow_plan(&control_flow, entry_key)
    })?;
    let state_dispatch = record_native_phase(&mut phase_timings, "state dispatch", || {
        build_state_dispatch_plan_with_workers(
            Arc::new(StateDispatchContext::from_runtime_flow(&runtime_flow)),
            runtime_state_inputs(&runtime_flow),
            workers.clone(),
        )
    });
    let entry_machine_name = control_flow
        .machine_by_symbol(entry_key.machine)
        .map(|machine| machine.name.as_str())
        .unwrap_or(ENTRY_MACHINE_NAME);
    let state_guards = record_native_phase(&mut phase_timings, "state guards", || {
        build_state_guard_plan(&state_dispatch, &control_flow, &layouts, entry_machine_name)
    });

    let mut native_plan = build_native_plan_skeleton(NativePlanSkeletonInput {
        target,
        host_abi,
        host_calls,
        control_flow,
        runtime_flow,
        state_dispatch,
        state_guards,
        layouts,
        entry_key,
        phase_timings,
    });
    let mut phase_timings = std::mem::take(&mut native_plan.phase_timings);
    native_plan.state_calls = record_native_phase(&mut phase_timings, "state calls", || {
        build_state_call_plan_with_workers(
            Arc::new(StateAnalysisContext::from_native_plan(&native_plan)),
            workers.clone(),
        )
    });
    native_plan.alias_flow = record_native_phase(&mut phase_timings, "alias flow", || {
        build_alias_flow_plan(&native_plan)
    });
    let state_analysis_context = Arc::new(StateAnalysisContext::from_native_plan(&native_plan));
    let state_storage_program = Arc::clone(&program);
    let state_values_program = Arc::clone(&program);
    let state_storage_context = Arc::clone(&state_analysis_context);
    let state_values_context = Arc::clone(&state_analysis_context);
    let state_storage_workers = workers.clone();
    let state_values_workers = workers.clone();
    let (state_storage, state_values) =
        record_native_phase(&mut phase_timings, "state storage/values", || {
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
    native_plan.state_storage = state_storage;
    native_plan.state_values = state_values;
    native_plan.runtime_bodies = record_native_phase(&mut phase_timings, "runtime bodies", || {
        build_runtime_dispatch_body_plan_with_workers(
            Arc::new(RuntimeDispatchBodyContext::from_native_plan(&native_plan)),
            native_plan
                .state_dispatch
                .states
                .iter()
                .map(|(_, dispatch_state)| dispatch_state.clone())
                .collect(),
            workers.clone(),
        )
    });
    let runtime_loop_context = Arc::new(RuntimeDispatchLoopContext::from_native_plan(&native_plan));
    let runtime_loop_inputs = runtime_dispatch_loop_inputs(&native_plan);
    let runtime_loop_workers = workers.clone();
    let runtime_storage_context = Arc::new(RuntimeStorageContext::from_native_plan(&native_plan));
    let runtime_storage_inputs = runtime_storage_body_inputs(&native_plan);
    let runtime_storage_workers = workers.clone();
    let (runtime_dispatch_loop, runtime_storage) =
        record_native_phase(&mut phase_timings, "runtime loop/storage", || {
            workers.join2(
                move || {
                    build_runtime_dispatch_loop_plan_with_workers(
                        runtime_loop_context,
                        runtime_loop_inputs,
                        runtime_loop_workers,
                    )
                },
                move || {
                    build_runtime_storage_plan_with_workers(
                        runtime_storage_context,
                        runtime_storage_inputs,
                        runtime_storage_workers,
                    )
                },
            )
        });
    native_plan.runtime_branching_calls =
        record_native_phase(&mut phase_timings, "runtime branching", || {
            build_runtime_branching_call_plan(&native_plan)
        });
    native_plan.runtime_dispatch_loop = runtime_dispatch_loop;
    native_plan.runtime_storage = runtime_storage;
    native_plan.runtime_text = record_native_phase(&mut phase_timings, "runtime text", || {
        build_runtime_text_plan(&native_plan)
    });
    native_plan.data = record_native_phase(&mut phase_timings, "native data", || {
        build_native_data_plan(&native_plan.host_calls, &native_plan.state_storage)
    });
    native_plan.instructions = record_native_phase(&mut phase_timings, "instructions", || {
        build_instruction_plan(&native_plan)
    });
    native_plan.machine_code = record_native_phase(&mut phase_timings, "machine code", || {
        build_machine_code_plan(&native_plan)
    })?;
    native_plan.object = record_native_phase(&mut phase_timings, "object plan", || {
        build_object_plan(&native_plan)
    })?;
    native_plan.relocations = record_native_phase(&mut phase_timings, "relocations", || {
        build_relocation_plan(&native_plan)
    })?;
    native_plan.phase_timings = phase_timings;

    Ok(native_plan)
}
