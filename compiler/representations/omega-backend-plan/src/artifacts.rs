use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendArtifactRoots {
    pub machine_instructions: MachineInstructionPlan,
    pub encoded_machine: EncodedMachinePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
}

impl BackendArtifactRoots {
    pub fn empty_for_target(target: NativeTarget) -> Self {
        Self {
            machine_instructions: MachineInstructionPlan::with_capacity(target, 0, 0),
            encoded_machine: EncodedMachinePlan::with_capacity(target, 0, 0, 0),
            object: ObjectPlan::with_capacity(target, 0, 0),
            relocations: RelocationPlan::with_target(target),
        }
    }
}
