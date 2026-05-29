use omega_assigned_target_operations::{AssignedInstructionOperand, AssignedOperation};

pub(crate) fn assign_operation(
    operation: &omega_target_operations::TargetOperation,
) -> AssignedOperation {
    AssignedOperation {
        kind: operation.kind.clone().into(),
        source_key: operation.source_key,
        source_statement: operation.source_statement,
    }
}

pub(crate) fn assign_instruction_operand(
    operand: &omega_target_operations::InstructionOperand,
) -> AssignedInstructionOperand {
    AssignedInstructionOperand {
        kind: operand.kind.clone().into(),
    }
}
