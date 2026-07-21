pub mod encoding;
mod entry;
pub mod operands;
mod selection;
pub mod widths;

pub use encoding::*;
pub use entry::derive_boundary_entry_storage_writes;
pub use operands::*;
pub use selection::build_instruction_plan;
pub use widths::*;

/// Re-exported for the relocation walker: the `CopyPlaces` site list is the
/// x86_64 materializer's own record of where its base movs sit.
pub use omega_isa_x86_64::{PlaceCopySide, PlaceCopySites};

use omega_abstract_operations::AbstractDataPlan;
use omega_checked_trees::CheckedTrees;
use omega_control_flow::{ControlFlowPlan, StateKey};
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
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionSelectionInput<'plan> {
    pub target: omega_target::NativeTarget,
    pub runtime_abi: &'plan RuntimeAbiPlan,
    pub entry_key: StateKey,
    pub entry_symbol: Arc<str>,
    pub program: &'plan CheckedTrees,
    pub control_flow: &'plan ControlFlowPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    /// See BackendPlan::receiver_bases (per-instance receiver dispatch).
    pub receiver_bases: &'plan [Option<usize>],
    /// See BackendPlan::state_contexts (same-context slot resolution).
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
