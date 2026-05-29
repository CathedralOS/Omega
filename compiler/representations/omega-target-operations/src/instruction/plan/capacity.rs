use crate::instruction::plan::TargetOperationPlan;
use crate::{TargetBoundarySummary, TargetOwnershipSummary, TargetValueSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

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
            values: TargetValueSummary::default(),
            boundary_edges: TargetBoundarySummary::default(),
            ownership: TargetOwnershipSummary::default(),
        }
    }
}
