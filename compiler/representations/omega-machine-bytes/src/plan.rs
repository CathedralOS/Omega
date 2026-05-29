use crate::{EncodedMachineFunction, EncodedMachineInstruction, EncodedMachineSemanticSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachineCode {
    pub functions: Arena<EncodedMachineFunction>,
    pub instructions: Arena<EncodedMachineInstruction>,
    pub bytes: Arena<u8>,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachinePlan {
    pub target: NativeTarget,
    pub code: EncodedMachineCode,
    pub semantics: EncodedMachineSemanticSummary,
}

impl Default for EncodedMachinePlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0)
    }
}

impl EncodedMachinePlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        byte_capacity: usize,
    ) -> Self {
        Self {
            target,
            code: EncodedMachineCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
                bytes: Arena::with_capacity(byte_capacity),
                byte_count: 0,
            },
            semantics: EncodedMachineSemanticSummary::with_capacity(0, 0, 0, 0, 0, 0),
        }
    }
}
