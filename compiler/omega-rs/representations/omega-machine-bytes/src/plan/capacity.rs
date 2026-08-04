use crate::{EncodedMachineCode, EncodedMachinePlan, EncodedMachineSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

impl EncodedMachinePlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        byte_capacity: usize,
    ) -> Self {
        Self::with_roots(
            target,
            EncodedMachineCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
                bytes: Arena::with_capacity(byte_capacity),
                runtime_value_operands: Arena::new(),
                byte_count: 0,
            },
            EncodedMachineSemanticSummary::with_capacity(0, 0, 0, 0, 0),
        )
    }
}
