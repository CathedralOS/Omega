use crate::{MachineFunction, MachineInstruction, MachineSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgramCode {
    pub functions: Arena<MachineFunction>,
    pub instructions: Arena<MachineInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProgram {
    pub target: NativeTarget,
    pub code: MachineProgramCode,
    pub semantics: MachineSemanticSummary,
}

impl MachineProgram {
    pub fn with_roots(
        target: NativeTarget,
        code: MachineProgramCode,
        semantics: MachineSemanticSummary,
    ) -> Self {
        Self {
            target,
            code,
            semantics,
        }
    }
}
