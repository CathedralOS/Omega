use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_checked_trees::CheckedTrees;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_instruction_selection::{InstructionSelectionInput, build_instruction_plan};
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_target::NativeTarget;
use omega_target_operations::TargetDataPlan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationLoweringInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub entry_symbol: Arc<str>,
    pub program: &'plan CheckedTrees,
    pub host_abi: &'plan HostAbiPlan,
    pub control_flow: &'plan ControlFlowPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub alias_flow: &'plan AliasFlowPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub layouts: &'plan LayoutPlan,
    pub data: &'plan TargetDataPlan,
}

impl<'plan> From<&'plan AbstractOperationLoweringInput<'plan>> for InstructionSelectionInput<'plan> {
    fn from(input: &'plan AbstractOperationLoweringInput<'plan>) -> Self {
        Self {
            target: input.target,
            entry_key: input.entry_key,
            entry_symbol: Arc::clone(&input.entry_symbol),
            program: input.program,
            host_abi: input.host_abi,
            control_flow: input.control_flow,
            host_calls: input.host_calls,
            state_calls: input.state_calls,
            alias_flow: input.alias_flow,
            state_storage: input.state_storage,
            runtime_flow: input.runtime_flow,
            runtime_bodies: input.runtime_bodies,
            runtime_branching_calls: input.runtime_branching_calls,
            runtime_dispatch_loop: input.runtime_dispatch_loop,
            runtime_storage: input.runtime_storage,
            runtime_text: input.runtime_text,
            state_guards: input.state_guards,
            layouts: input.layouts,
            data: input.data,
        }
    }
}

pub fn build_abstract_operation_plan(input: &AbstractOperationLoweringInput<'_>) -> AbstractOperationPlan {
    build_instruction_plan(&InstructionSelectionInput::from(input)).into()
}
