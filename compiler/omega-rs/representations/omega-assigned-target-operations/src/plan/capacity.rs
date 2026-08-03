use crate::{AssignedSemanticSummary, AssignedTargetOperationPlan};
use omega_target::NativeTarget;
use psi_arena::Arena;

impl AssignedTargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
        host_binding_capacity: usize,
    ) -> Self {
        Self::with_roots(
            target,
            crate::AssignedTargetOperationCode {
                functions: Arena::with_capacity(function_capacity),
                instructions: Arena::with_capacity(instruction_capacity),
                operands: Arena::with_capacity(operand_capacity),
                runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
                host_bindings: Arena::with_capacity(host_binding_capacity),
            },
            AssignedSemanticSummary::with_capacity(0, 0, 0, 0, 0),
        )
    }
}
