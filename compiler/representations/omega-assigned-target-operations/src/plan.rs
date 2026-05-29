mod capacity;
mod conversions;
mod lookups;
mod operand_handles;

use crate::{
    AssignedInstructionOperand, AssignedOperation, AssignedSemanticSummary,
    AssignedTargetOperationFunction, AssignedValueOperand,
};
use omega_core::arena::Arena;
use omega_target::NativeTarget;
use omega_target_operations::TargetHostBinding;

pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
    pub semantics: AssignedSemanticSummary,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0)
    }
}
