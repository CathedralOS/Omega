use crate::abi::{HostAbiPlan, build_host_abi_plan};
use crate::alias_flow::{AliasFlowPlan, build_alias_flow_plan};
use crate::control_flow::{ControlFlowPlan, build_control_flow_plan_with_workers};
use crate::data::{NativeDataPlan, build_native_data_plan};
use crate::host_calls::{HostCallPlan, build_host_call_plan_with_workers};
use crate::instructions::{InstructionPlan, build_instruction_plan};
use crate::layout::{LayoutPlan, build_layout_plan};
use crate::machine_code::{MachineCodePlan, build_machine_code_plan};
use crate::object::{ObjectPlan, build_object_plan};
use crate::relocations::{RelocationPlan, build_relocation_plan};
use crate::runtime_dispatch::bodies::{RuntimeDispatchBodyPlan, build_runtime_dispatch_body_plan};
use crate::runtime_dispatch::branching::{
    RuntimeBranchingCallPlan, build_runtime_branching_call_plan,
};
use crate::runtime_dispatch::loop_plan::{
    RuntimeDispatchLoopPlan, build_runtime_dispatch_loop_plan,
};
use crate::runtime_flow::{RuntimeFlowPlan, build_runtime_flow_plan};
use crate::runtime_storage::{RuntimeStoragePlan, build_runtime_storage_plan};
use crate::runtime_text::{RuntimeTextPlan, build_runtime_text_plan};
use crate::state_analysis::StateAnalysisContext;
use crate::state_calls::{StateCallPlan, build_state_call_plan_with_workers};
use crate::state_dispatch::{StateDispatchPlan, build_state_dispatch_plan};
use crate::state_guards::{StateGuardPlan, build_state_guard_plan};
use crate::state_storage::{StateStoragePlan, build_state_storage_plan_with_workers};
use crate::state_values::{StateValuePlan, build_state_value_plan_with_workers};
use crate::target::NativeTarget;
use omega_core::diagnostics::Diagnostic;
use omega_core::parallel::{WorkerPool, WorkerPoolHandle};
use omega_typed_program::Program;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePlan {
    pub target: NativeTarget,
    pub host_abi: HostAbiPlan,
    pub host_calls: HostCallPlan,
    pub state_calls: StateCallPlan,
    pub alias_flow: AliasFlowPlan,
    pub state_storage: StateStoragePlan,
    pub state_values: StateValuePlan,
    pub data: NativeDataPlan,
    pub instructions: InstructionPlan,
    pub control_flow: ControlFlowPlan,
    pub runtime_flow: RuntimeFlowPlan,
    pub state_dispatch: StateDispatchPlan,
    pub state_guards: StateGuardPlan,
    pub runtime_bodies: RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: RuntimeDispatchLoopPlan,
    pub runtime_storage: RuntimeStoragePlan,
    pub runtime_text: RuntimeTextPlan,
    pub layouts: LayoutPlan,
    pub machine_code: MachineCodePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
    pub entry_machine: String,
    pub entry_state: String,
}

pub fn build_native_plan(
    program: &Program,
    target: NativeTarget,
) -> Result<NativePlan, Diagnostic> {
    let workers = WorkerPool::with_available_parallelism();

    build_native_plan_with_workers(Arc::new(program.clone()), target, workers.handle())
}

pub fn build_native_plan_with_workers(
    program: Arc<Program>,
    target: NativeTarget,
    workers: WorkerPoolHandle,
) -> Result<NativePlan, Diagnostic> {
    let entry_machine = "main".to_owned();
    let entry_state = "entry".to_owned();
    let host_abi = build_host_abi_plan(target);
    let host_call_program = Arc::clone(&program);
    let layout_program = Arc::clone(&program);
    let control_flow_program = Arc::clone(&program);
    let host_call_abi = Arc::new(host_abi.clone());
    let control_flow_workers = workers.clone();
    let host_call_workers = workers.clone();
    let (control_flow, layouts, host_calls) = workers.join3(
        move || build_control_flow_plan_with_workers(control_flow_program, control_flow_workers),
        move || build_layout_plan(&layout_program, target),
        move || {
            build_host_call_plan_with_workers(
                host_call_program,
                target,
                host_call_abi,
                host_call_workers,
            )
        },
    );
    let control_flow = control_flow?;
    let layouts = layouts?;
    let host_calls = host_calls?;
    let runtime_flow = build_runtime_flow_plan(&control_flow, &entry_machine, &entry_state)?;
    let state_dispatch = build_state_dispatch_plan(&runtime_flow);
    let state_guards = build_state_guard_plan(&state_dispatch, &layouts, &entry_machine);

    let mut native_plan = NativePlan {
        target,
        host_abi,
        host_calls,
        state_calls: StateCallPlan::default(),
        alias_flow: AliasFlowPlan::default(),
        state_storage: StateStoragePlan::default(),
        state_values: StateValuePlan::default(),
        data: NativeDataPlan::default(),
        instructions: InstructionPlan {
            target,
            functions: omega_core::arena::Arena::new(),
            instructions: omega_core::arena::Arena::new(),
            operands: omega_core::arena::Arena::new(),
        },
        control_flow,
        runtime_flow,
        state_dispatch,
        state_guards,
        runtime_bodies: RuntimeDispatchBodyPlan::default(),
        runtime_branching_calls: RuntimeBranchingCallPlan::default(),
        runtime_dispatch_loop: RuntimeDispatchLoopPlan::default(),
        runtime_storage: RuntimeStoragePlan::default(),
        runtime_text: RuntimeTextPlan::default(),
        layouts,
        machine_code: MachineCodePlan::default(),
        object: ObjectPlan {
            target,
            sections: omega_core::arena::Arena::new(),
            symbols: omega_core::arena::Arena::new(),
            entry_symbol: String::new(),
        },
        relocations: RelocationPlan {
            target,
            records: omega_core::arena::Arena::new(),
        },
        entry_machine,
        entry_state,
    };
    native_plan.state_calls = build_state_call_plan_with_workers(
        Arc::new(StateAnalysisContext::from_native_plan(&native_plan)),
        workers.clone(),
    );
    native_plan.alias_flow = build_alias_flow_plan(&native_plan);
    let state_analysis_context = Arc::new(StateAnalysisContext::from_native_plan(&native_plan));
    let state_storage_program = Arc::clone(&program);
    let state_values_program = Arc::clone(&program);
    let state_storage_context = Arc::clone(&state_analysis_context);
    let state_values_context = Arc::clone(&state_analysis_context);
    let state_storage_workers = workers.clone();
    let state_values_workers = workers.clone();
    let (state_storage, state_values) = workers.join2(
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
    );
    native_plan.state_storage = state_storage;
    native_plan.state_values = state_values;
    native_plan.runtime_bodies = build_runtime_dispatch_body_plan(&native_plan);
    native_plan.runtime_branching_calls = build_runtime_branching_call_plan(&native_plan);
    native_plan.runtime_dispatch_loop = build_runtime_dispatch_loop_plan(&native_plan);
    native_plan.runtime_storage = build_runtime_storage_plan(&native_plan);
    native_plan.runtime_text = build_runtime_text_plan(&native_plan);
    native_plan.data = build_native_data_plan(&native_plan.host_calls, &native_plan.state_storage);
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.instructions = build_instruction_plan(&native_plan);
    native_plan.machine_code = build_machine_code_plan(&native_plan)?;
    native_plan.object = build_object_plan(&native_plan)?;
    native_plan.relocations = build_relocation_plan(&native_plan)?;

    Ok(native_plan)
}
