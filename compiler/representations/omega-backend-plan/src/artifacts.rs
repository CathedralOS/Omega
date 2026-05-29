use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_object_file::{ObjectPlan, RelocationPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendArtifactRoots {
    pub machine_instructions: MachineInstructionPlan,
    pub encoded_machine: EncodedMachinePlan,
    pub object: ObjectPlan,
    pub relocations: RelocationPlan,
}
