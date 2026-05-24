use omega_assigned_target_operations::{
    AssignedOperation, AssignedRegisterBank, AssignedRegisterName, AssignedTargetOperationPlan,
    AssignedValueHomeKind, AssignedValueOperand, X86_64AssignedRegister,
    assigned_operation_span_from_target,
};
use omega_target::Architecture;
use omega_target_operations::TargetOperationPlan;

pub fn build_assigned_target_operations(
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
            .insert(omega_assigned_target_operations::AssignedTargetOperationFunction {
                symbol: std::sync::Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: assigned_operation_span_from_target(function.instructions),
            });
    }

    for (_, instruction) in target_operations.instructions.iter() {
        assigned_target_operations.instructions.insert(AssignedOperation {
            kind: instruction.kind.clone(),
            source_key: instruction.source_key,
            source_statement: instruction.source_statement,
        });
    }
    for (_, operand) in target_operations.operands.iter() {
        assigned_target_operations
            .operands
            .insert(omega_assigned_target_operations::AssignedInstructionOperand {
                kind: operand.kind.clone().into(),
            });
    }
    assigned_target_operations.host_bindings = target_operations.host_bindings.clone();

    let mut next_scratch_slot = 0u16;
    for (_, operand) in target_operations.runtime_value_operands.iter() {
        let kind = match operand {
            omega_target_operations::TargetValueOperand::Immediate(_) => {
                AssignedValueHomeKind::Immediate
            }
            omega_target_operations::TargetValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => match region {
                omega_target_operations::RuntimeStorageRegion::Machine => {
                    AssignedValueHomeKind::RuntimeStorage {
                        region: *region,
                        byte_offset: *byte_offset,
                        byte_size: *byte_size,
                    }
                }
                omega_target_operations::RuntimeStorageRegion::RuntimeFrame => {
                    AssignedValueHomeKind::StackSlot {
                        byte_offset: *byte_offset,
                        byte_size: *byte_size,
                    }
                }
            },
            omega_target_operations::TargetValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => AssignedValueHomeKind::RuntimePointee {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameIndexed {
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
            omega_target_operations::TargetValueOperand::Binary { .. } => {
                let name = scratch_register_name(target_operations.target.architecture, next_scratch_slot);
                next_scratch_slot = next_scratch_slot.saturating_add(1);
                AssignedValueHomeKind::ScratchRegister {
                    bank: AssignedRegisterBank::GeneralPurpose,
                    name,
                }
            }
        };

        assigned_target_operations
            .runtime_value_operands
            .insert(AssignedValueOperand {
                kind: operand.clone().into(),
                home: kind,
            });
    }

    assigned_target_operations
}

fn scratch_register_name(architecture: Architecture, slot: u16) -> AssignedRegisterName {
    match architecture {
        Architecture::Aarch64 => {
            let register = 19u8.saturating_add((slot % 9) as u8);
            AssignedRegisterName::Aarch64X(register)
        }
        Architecture::X86_64 => AssignedRegisterName::X86_64(match slot % 6 {
            0 => X86_64AssignedRegister::R10,
            1 => X86_64AssignedRegister::R11,
            2 => X86_64AssignedRegister::R12,
            3 => X86_64AssignedRegister::R13,
            4 => X86_64AssignedRegister::R14,
            _ => X86_64AssignedRegister::R15,
        }),
    }
}
