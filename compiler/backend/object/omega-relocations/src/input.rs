use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_machine_bytes::EncodedMachinePlan;
use omega_object_file::ObjectPlan;
use omega_target::NativeTarget;
use omega_target_operations::{InstructionPlan, TargetDataPlan};

#[derive(Debug, Clone, Copy)]
pub struct RelocationPlanningInput<'plan> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub assigned_target_operations: &'plan AssignedTargetOperationPlan,
    pub encoded_machine: &'plan EncodedMachinePlan,
    pub data: &'plan TargetDataPlan,
    pub object: &'plan ObjectPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub entry_machine_name: &'plan str,
}
