use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_target::NativeTarget;

mod branch_distances;
mod code;
mod emitter;
mod encoding;
mod host_bindings;
mod instruction_bytes;
mod layout;
mod selected_instruction_queries;
mod semantics;
pub use emitter::{MachineEmissionInput, emit_machine_bytes};

#[derive(Debug, Clone, Copy)]
pub(crate) struct MachineEmissionContext<'plan> {
    pub target: NativeTarget,
    pub assigned_target_operations: &'plan AssignedTargetOperationPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub data: &'plan omega_target_operations::TargetDataPlan,
    pub terminal_dispatch_index: u32,
}
