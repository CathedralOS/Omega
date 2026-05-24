use omega_assigned_target_operations::{
    AssignedRegisterBank, AssignedTargetOperationPlan, AssignedValueHome, AssignedValueHomeKind,
};
use omega_target_operations::InstructionPlan;

pub fn build_assigned_target_operations(
    target_operations: &InstructionPlan,
) -> AssignedTargetOperationPlan {
    let mut assigned_target_operations = AssignedTargetOperationPlan::with_capacity(
        target_operations.target,
        target_operations.functions.len(),
        target_operations.instructions.len(),
        target_operations.operands.len(),
        target_operations.runtime_value_operands.len(),
        target_operations.runtime_value_operands.len(),
    );

    for (_, function) in target_operations.functions.iter() {
        assigned_target_operations
            .functions
            .insert(omega_assigned_target_operations::AssignedTargetOperationFunction {
                symbol: std::sync::Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: function.instructions,
            });
    }

    assigned_target_operations.instructions = target_operations.instructions.clone();
    assigned_target_operations.operands = target_operations.operands.clone();
    assigned_target_operations.runtime_value_operands =
        target_operations.runtime_value_operands.clone();

    let mut next_scratch_slot = 0u16;
    for (_, operand) in target_operations.runtime_value_operands.iter() {
        let kind = match operand {
            omega_target_operations::RuntimeValueOperand::Immediate(_) => {
                AssignedValueHomeKind::Immediate
            }
            omega_target_operations::RuntimeValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => AssignedValueHomeKind::RuntimeStorage {
                region: *region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            omega_target_operations::RuntimeValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => AssignedValueHomeKind::RuntimePointee {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
            },
            omega_target_operations::RuntimeValueOperand::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => AssignedValueHomeKind::RuntimeFrameIndexed {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
            },
            omega_target_operations::RuntimeValueOperand::Binary { .. } => {
                let slot = next_scratch_slot;
                next_scratch_slot = next_scratch_slot.saturating_add(1);
                AssignedValueHomeKind::ScratchRegister {
                    bank: AssignedRegisterBank::GeneralPurpose,
                    slot,
                }
            }
        };

        assigned_target_operations
            .runtime_value_homes
            .insert(AssignedValueHome { kind });
    }

    assigned_target_operations
}
