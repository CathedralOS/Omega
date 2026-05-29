use crate::{MachineFunction, MachineInstruction, MachineSemanticSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

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

impl Default for MachineProgram {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0)
    }
}

impl MachineProgram {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self {
            target,
            code: MachineProgramCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
            },
            semantics: MachineSemanticSummary::with_capacity(0, 0, 0, 0, 0, 0),
        }
    }
}
