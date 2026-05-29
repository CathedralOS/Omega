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
