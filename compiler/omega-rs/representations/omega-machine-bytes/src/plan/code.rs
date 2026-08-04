use crate::{EncodedMachineFunction, EncodedMachineInstruction, EncodedMachineSemanticSummary};
use omega_target::NativeTarget;
use psi_arena::Arena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachineCode {
    pub functions: Arena<EncodedMachineFunction>,
    pub instructions: Arena<EncodedMachineInstruction>,
    pub bytes: Arena<u8>,
    /// Canonical operand arena retained for final replay of recursive
    /// compiler-owned value evaluators. Handles in instruction recipes point
    /// into this 1:1 arena copy.
    pub runtime_value_operands: Arena<omega_target_operations::RuntimeValueOperand>,
    pub byte_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedMachinePlan {
    pub target: NativeTarget,
    pub code: EncodedMachineCode,
    pub semantics: EncodedMachineSemanticSummary,
}

impl EncodedMachinePlan {
    pub fn with_roots(
        target: NativeTarget,
        code: EncodedMachineCode,
        semantics: EncodedMachineSemanticSummary,
    ) -> Self {
        Self {
            target,
            code,
            semantics,
        }
    }
}
