use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_target_operations::TargetOperationPlan;

use crate::{functions, operations, values};

pub(crate) fn build_assigned_target_operations(
    target_operations: &TargetOperationPlan,
) -> AssignedTargetOperationPlan {
    let mut assigned_target_operations = AssignedTargetOperationPlan::with_capacity(
        target_operations.target,
        target_operations.functions.len(),
        target_operations.instructions.len(),
        target_operations.operands.len(),
        target_operations.runtime_value_operands.len(),
        target_operations.host_bindings.len(),
    );

    for (_, function) in target_operations.functions.iter() {
        assigned_target_operations
            .functions
            .insert(functions::assign_function(function));
    }

    for (_, instruction) in target_operations.instructions.iter() {
        assigned_target_operations
            .instructions
            .insert(operations::assign_operation(instruction));
    }
    for (_, operand) in target_operations.operands.iter() {
        assigned_target_operations
            .operands
            .insert(operations::assign_instruction_operand(operand));
    }
    assigned_target_operations.host_bindings = target_operations.host_bindings.clone();
    assigned_target_operations.semantics.values = target_operations.semantics.values.clone();
    assigned_target_operations.semantics.boundary_edges =
        target_operations.semantics.boundary_edges.clone();
    assigned_target_operations.semantics.ownership = target_operations.semantics.ownership.clone();

    values::assign_runtime_value_operands(target_operations, &mut assigned_target_operations);

    assigned_target_operations
}
