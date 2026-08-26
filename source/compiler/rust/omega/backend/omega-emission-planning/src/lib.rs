use omega_backend_report_types::{EmissionBlocker, emission_blocker};
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_layout::LayoutPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::StateCallPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_operations::{InstructionPlan, TargetDataPlan};

mod authored_import_blockers;
mod builder;
mod call_result_blockers;
mod contained_receiver_blockers;
mod descriptor_argument_blockers;
mod dispatch_route;
mod guard_expression_support;
mod host_argument_blockers;
mod host_binding_blockers;
mod reentrant_value_call_blockers;
mod required_emission_verification;
mod runtime_dispatch_blockers;
mod runtime_text_blockers;
mod selected_instruction_queries;
mod semantic_scope;
mod state_call_blockers;
mod state_codegen_blockers;
mod state_guard_blockers;
mod storage_blockers;
mod unlowered_guard_blockers;
mod value_call_arm_effect_blockers;

pub use builder::build_emission_plan;

pub struct EmissionPlanningInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub host_abi: &'plan HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    /// See BackendPlan::receiver_bases (per-instance receiver dispatch).
    pub receiver_bases: &'plan [Option<usize>],
    /// See BackendPlan::state_contexts (same-context slot resolution).
    pub state_contexts: &'plan [u32],
    pub state_storage: &'plan StateStoragePlan,
    pub state_values: &'plan StateValuePlan,
    pub data: &'plan TargetDataPlan,
    pub instructions: &'plan InstructionPlan,
    pub control_flow: &'plan ControlFlowPlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub layouts: &'plan LayoutPlan,
    pub machine_instructions: &'plan MachineInstructionPlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub object: &'plan ObjectPlan,
    pub relocations: &'plan RelocationPlan,
}

fn blocker(stage: &str, reason: &str) -> EmissionBlocker {
    emission_blocker(stage, reason)
}
