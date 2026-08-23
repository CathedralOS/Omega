use crate::{MachineProgram, MachineProgramCode, MachineSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

impl MachineProgram {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self::with_roots(
            target,
            MachineProgramCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
            },
            MachineSemanticSummary::with_capacity(0, 0, 0, 0, 0),
        )
    }
}
