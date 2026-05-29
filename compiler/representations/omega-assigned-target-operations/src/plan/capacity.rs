use crate::{AssignedTargetOperationPlan, AssignedValueSummary};
use omega_core::arena::Arena;
use omega_target::NativeTarget;

impl AssignedTargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
        host_binding_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            host_bindings: Arena::with_capacity(host_binding_capacity),
            values: AssignedValueSummary::default(),
            boundary_edges: omega_target_operations::TargetBoundarySummary::default(),
            ownership: omega_target_operations::TargetOwnershipSummary::default(),
        }
    }
}
