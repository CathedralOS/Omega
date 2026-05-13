pub mod encoding;
pub mod operands;
mod selection;
pub mod widths;

pub use encoding::*;
pub use operands::*;
pub use selection::build_instruction_plan;
pub use widths::*;

use omega_calling_conventions::HostAbiPlan;
use omega_checked_trees::Program;
use omega_control_flow::{ControlFlowPlan, StateKey};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionSelectionInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub entry_symbol: String,
    pub program: &'plan Program,
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
