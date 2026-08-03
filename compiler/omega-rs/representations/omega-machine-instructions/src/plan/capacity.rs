use crate::{MachineInstructionCode, MachineInstructionPlan, MachineInstructionSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

impl MachineInstructionPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
    ) -> Self {
        Self::with_roots(
            target,
            MachineInstructionCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
            },
            MachineInstructionSemanticSummary::with_capacity(0, 0, 0, 0, 0),
        )
    }
}
