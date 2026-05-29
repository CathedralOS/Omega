mod capacity;
mod conversions;
mod lookups;
mod operand_handles;

use crate::{
    AssignedInstructionOperand, AssignedOperation, AssignedTargetOperationFunction,
    AssignedValueOperand,
};
use omega_core::arena::Arena;
use omega_target::NativeTarget;
use omega_target_operations::TargetHostBinding;

pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;
pub type AssignedValueSummary = omega_target_operations::TargetValueSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
    pub values: AssignedValueSummary,
    pub boundary_edges: omega_target_operations::TargetBoundarySummary,
    pub ownership: omega_target_operations::TargetOwnershipSummary,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0)
    }
}
