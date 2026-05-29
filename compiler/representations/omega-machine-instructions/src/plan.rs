use crate::{MachineInstruction, MachineInstructionFunction, MachineInstructionValueSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<MachineInstructionFunction>,
    pub instructions: Arena<MachineInstruction>,
    pub values: MachineInstructionValueSummary,
    pub boundary_edges: omega_target_operations::TargetBoundarySummary,
    pub ownership: omega_target_operations::TargetOwnershipSummary,
}

impl Default for MachineInstructionPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0)
    }
}

impl MachineInstructionPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            values: MachineInstructionValueSummary::default(),
            boundary_edges: omega_target_operations::TargetBoundarySummary::default(),
            ownership: omega_target_operations::TargetOwnershipSummary::default(),
        }
    }
}
