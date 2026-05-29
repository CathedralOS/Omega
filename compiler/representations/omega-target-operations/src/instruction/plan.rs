mod capacity;
mod lookups;

use super::{
    InstructionOperand, TargetBoundarySummary, TargetHostBinding, TargetOwnershipSummary,
    TargetValueSummary,
};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<super::TargetOperationFunction>,
    pub instructions: Arena<super::TargetOperation>,
    pub operands: Arena<InstructionOperand>,
    pub runtime_value_operands: Arena<super::TargetValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
    pub values: TargetValueSummary,
    pub boundary_edges: TargetBoundarySummary,
    pub ownership: TargetOwnershipSummary,
}

pub type InstructionPlan = TargetOperationPlan;

impl Default for TargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}
