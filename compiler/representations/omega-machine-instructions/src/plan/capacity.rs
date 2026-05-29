use crate::{MachineInstructionCode, MachineInstructionPlan, MachineInstructionSemanticSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

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
