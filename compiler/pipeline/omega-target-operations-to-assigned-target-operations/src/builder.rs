use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::TargetOperationPlan;

use crate::{functions, operations, values};

pub(crate) fn build_assigned_target_operations(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationPlan {
    let mut assigned_target_operations = AssignedTargetOperationPlan::with_capacity(
        target_operations.target,
        target_operations.code.functions.len(),
        target_operations.code.instructions.len(),
        target_operations.code.operands.len(),
        target_operations.code.runtime_value_operands.len(),
        target_operations.code.host_bindings.len(),
    );

    for (_, function) in target_operations.code.functions.iter() {
        assigned_target_operations
            .code
            .functions
            .insert(functions::assign_function(function));
    }

    for (_, instruction) in target_operations.code.instructions.iter() {
        assigned_target_operations
            .code
            .instructions
            .insert(operations::assign_operation(instruction));
    }
    for (_, operand) in target_operations.code.operands.iter() {
        assigned_target_operations
            .code
            .operands
            .insert(operations::assign_instruction_operand(operand));
    }
    assigned_target_operations.code.host_bindings = target_operations.code.host_bindings.clone();
    assigned_target_operations.semantics = target_operations.semantics.clone();

    values::assign_runtime_value_operands(target_operations, &mut assigned_target_operations);

    assigned_target_operations
}
