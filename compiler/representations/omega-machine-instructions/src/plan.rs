use crate::{MachineInstruction, MachineInstructionFunction, MachineInstructionSemanticSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionCode {
    pub functions: Arena<MachineInstructionFunction>,
    pub instructions: Arena<MachineInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstructionPlan {
    pub target: NativeTarget,
    pub code: MachineInstructionCode,
    pub semantics: MachineInstructionSemanticSummary,
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
            code: MachineInstructionCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
            },
            semantics: MachineInstructionSemanticSummary::with_capacity(0, 0, 0, 0, 0, 0),
        }
    }
}
