mod capacity;
mod lookups;

use super::{InstructionOperand, TargetHostBinding, TargetSemanticSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationCode {
    pub functions: Arena<super::TargetOperationFunction>,
    pub instructions: Arena<super::TargetOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<super::TargetValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub target: NativeTarget,
    pub code: TargetOperationCode,
    pub semantics: TargetSemanticSummary,
}

pub type InstructionPlan = TargetOperationPlan;

impl Default for TargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}
