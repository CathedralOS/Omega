use crate::{MachineInstruction, MachineInstructionFunction, MachineInstructionSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

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

impl MachineInstructionPlan {
    pub fn with_roots(
        target: NativeTarget,
        code: MachineInstructionCode,
        semantics: MachineInstructionSemanticSummary,
    ) -> Self {
        Self {
            target,
            code,
            semantics,
        }
    }
}
