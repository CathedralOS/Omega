use omega_abstract_operations::AbstractOperationPlan;
use omega_target::NativeTarget;
use omega_target_operations::InstructionPlan;

pub fn build_target_operation_plan(
    target: NativeTarget,
    abstract_operations: &AbstractOperationPlan,
) -> InstructionPlan {
    InstructionPlan {
        target,
        functions: abstract_operations.functions.clone(),
        instructions: abstract_operations.instructions.clone(),
        operands: abstract_operations.operands.clone(),
        runtime_value_operands: abstract_operations.runtime_value_operands.clone(),
    }
}
