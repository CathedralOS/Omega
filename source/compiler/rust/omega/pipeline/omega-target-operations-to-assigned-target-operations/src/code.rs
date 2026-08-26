use omega_assigned_target_operations::{AssignedTargetOperationCode, AssignedTargetOperationPlan};
use omega_target_operations::TargetOperationPlan;

use crate::{functions, operations, values};

pub(crate) fn build_assigned_target_operation_code(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationCode {
    let AssignedTargetOperationPlan { mut code, .. } = AssignedTargetOperationPlan::with_capacity(
        target_operations.target,
        target_operations.code.functions.len(),
        target_operations.code.instructions.len(),
        target_operations.code.operands.len(),
        target_operations.code.runtime_value_operands.len(),
        target_operations.code.host_bindings.len(),
    );

    code.host_bindings = target_operations.code.host_bindings.clone();

    for (_, function) in target_operations.code.functions.iter() {
        code.functions.insert(functions::assign_function(function));
    }

    for (_, instruction) in target_operations.code.instructions.iter() {
        code.instructions
            .insert(operations::assign_operation(instruction));
    }

    for (_, operand) in target_operations.code.operands.iter() {
        code.operands
            .insert(operations::assign_instruction_operand(operand));
    }

    values::assign_runtime_value_operands(target_operations, &mut code);

    code
}
