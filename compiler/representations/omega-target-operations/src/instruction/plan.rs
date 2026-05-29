use super::{
    HostOperationKey, InstructionOperand, TargetBoundarySummary, TargetHostBinding,
    TargetOwnershipSummary,
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
    pub boundary_edges: TargetBoundarySummary,
    pub ownership: TargetOwnershipSummary,
}

pub type InstructionPlan = TargetOperationPlan;

impl Default for TargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0)
    }
}

impl TargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            host_bindings: Arena::new(),
            boundary_edges: TargetBoundarySummary::default(),
            ownership: TargetOwnershipSummary::default(),
        }
    }

    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }
}
