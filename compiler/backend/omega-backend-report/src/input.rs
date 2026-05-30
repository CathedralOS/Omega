use omega_abstract_operations::AbstractOperationPlan;
use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_control_flow::{ControlFlowPlan, StateKey};
use omega_core::allocations::AllocationDelta;
use omega_layout::LayoutPlan;
use omega_machine_bytes::{
    EncodedMachineBoundarySummary, EncodedMachineOwnershipSummary, EncodedMachinePlan,
    EncodedMachineSemanticSummary, EncodedMachineValueSummary,
};
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_platform_interface::HostCallPlan;
use omega_runtime_bodies::RuntimeDispatchBodyPlan;
use omega_runtime_branching::RuntimeBranchingCallPlan;
use omega_runtime_dispatch_loop::RuntimeDispatchLoopPlan;
use omega_runtime_storage::RuntimeStoragePlan;
use omega_runtime_text::RuntimeTextPlan;
use omega_state_calls::{AliasFlowPlan, StateCallPlan};
use omega_state_dispatch::StateDispatchPlan;
use omega_state_graph::RuntimeFlowPlan;
use omega_state_guards::StateGuardPlan;
use omega_state_storage::StateStoragePlan;
use omega_state_values::StateValuePlan;
use omega_target::NativeTarget;
use omega_target_operations::{TargetDataPlan, TargetOperationPlan};

pub struct BackendReportPhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

pub struct BackendReportInput<'plan> {
    pub target: NativeTarget,
    pub entry_key: StateKey,
    pub phase_timings: &'plan [BackendReportPhaseTiming],
    pub host_abi: &'plan HostAbiPlan,
    pub host_calls: &'plan HostCallPlan,
    pub state_calls: &'plan StateCallPlan,
    pub alias_flow: &'plan AliasFlowPlan,
    pub state_storage: &'plan StateStoragePlan,
    pub state_values: &'plan StateValuePlan,
    pub data: &'plan TargetDataPlan,
    pub abstract_operations: &'plan AbstractOperationPlan,
    pub target_operations: &'plan TargetOperationPlan,
    pub assigned_target_operations: &'plan AssignedTargetOperationPlan,
    pub control_flow: &'plan ControlFlowPlan,
    pub runtime_flow: &'plan RuntimeFlowPlan,
    pub state_dispatch: &'plan StateDispatchPlan,
    pub state_guards: &'plan StateGuardPlan,
    pub runtime_bodies: &'plan RuntimeDispatchBodyPlan,
    pub runtime_branching_calls: &'plan RuntimeBranchingCallPlan,
    pub runtime_dispatch_loop: &'plan RuntimeDispatchLoopPlan,
    pub runtime_storage: &'plan RuntimeStoragePlan,
    pub runtime_text: &'plan RuntimeTextPlan,
    pub layouts: &'plan LayoutPlan,
    pub machine_instructions: &'plan MachineInstructionPlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub object: &'plan ObjectPlan,
    pub relocations: &'plan RelocationPlan,
}

impl<'plan> BackendReportInput<'plan> {
    pub fn entry_machine_name(&self) -> &str {
        self.control_flow
            .machine_by_symbol(self.entry_key.machine)
            .map(|machine| machine.name.as_str())
            .unwrap_or("")
    }

    pub fn entry_state_name(&self) -> &str {
        self.control_flow
            .state_by_key(self.entry_key)
            .map(|state| state.name.as_str())
            .unwrap_or("")
    }

    pub fn semantic_summary(&self) -> &EncodedMachineSemanticSummary {
        &self.encoded_machine.semantics
    }

    pub fn value_summary(&self) -> &EncodedMachineValueSummary {
        &self.encoded_machine.semantics.values
    }

    pub fn boundary_summary(&self) -> &EncodedMachineBoundarySummary {
        &self.encoded_machine.semantics.boundaries
    }

    pub fn ownership_summary(&self) -> &EncodedMachineOwnershipSummary {
        &self.encoded_machine.semantics.ownership
    }
}
