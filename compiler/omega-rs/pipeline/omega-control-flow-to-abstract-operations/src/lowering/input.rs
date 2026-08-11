use omega_abstract_operations::AbstractDataPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_instruction_selection::InstructionSelectionInput;
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_runtime_abi::RuntimeAbiPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use psi_checked_trees::CheckedTrees;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationLoweringInput<'plan> {
    pub target: omega_target::NativeTarget,
    pub freestanding: bool,
    pub runtime_abi: &'plan RuntimeAbiPlan,
    pub entry_key: StateKey,
    pub entry_boundary_plan: Option<&'plan omega_calling_conventions::BoundaryEntryPlan>,
    pub entry_symbol: Arc<str>,
    pub program: &'plan CheckedTrees,
    pub selected_provider_plans: &'plan omega_effects::SelectedProviderPlanFacts,
    pub control_flow: &'plan ControlFlowPlan,
    pub host_abi: &'plan omega_calling_conventions::HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub receiver_bases: &'plan [Option<usize>],
    pub state_contexts: &'plan [u32],
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
    pub data: &'plan AbstractDataPlan,
}

impl<'plan> From<&'plan AbstractOperationLoweringInput<'plan>>
    for InstructionSelectionInput<'plan>
{
    fn from(input: &'plan AbstractOperationLoweringInput<'plan>) -> Self {
        Self {
            target: input.target,
            freestanding: input.freestanding,
            runtime_abi: input.runtime_abi,
            entry_key: input.entry_key,
            entry_boundary_plan: input.entry_boundary_plan,
            entry_symbol: Arc::clone(&input.entry_symbol),
            program: input.program,
            selected_provider_plans: input.selected_provider_plans,
            control_flow: input.control_flow,
            host_abi: input.host_abi,
            host_calls: input.host_calls,
            state_calls: input.state_calls,
            receiver_bases: input.receiver_bases,
            state_contexts: input.state_contexts,
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
