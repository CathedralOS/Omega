use omega_assigned_target_operations::{
    AssignedRegisterBank, AssignedTargetOperationCode, AssignedValueHomeKind, AssignedValueOperand,
};
use omega_target_operations::{RuntimeStorageRegion, TargetOperationPlan, TargetValueOperand};

use crate::registers;

pub(crate) fn assign_runtime_value_operands(
    target_operations: &TargetOperationPlan,
    target: &mut AssignedTargetOperationCode,
) {
    let mut next_scratch_slot = 0u16;
    for (_, operand) in target_operations.code.runtime_value_operands.iter() {
        let home = assign_value_home(target_operations, operand, &mut next_scratch_slot);
        target.runtime_value_operands.insert(AssignedValueOperand {
            kind: operand.clone().into(),
            home,
        });
    }
}

fn assign_value_home(
    target_operations: &TargetOperationPlan,
    operand: &TargetValueOperand,
    next_scratch_slot: &mut u16,
) -> AssignedValueHomeKind {
    match operand {
        TargetValueOperand::Immediate(_) => AssignedValueHomeKind::Immediate,
        TargetValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => match region {
            RuntimeStorageRegion::Machine => AssignedValueHomeKind::RuntimeStorage {
                region: *region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            RuntimeStorageRegion::RuntimeFrame => AssignedValueHomeKind::StackSlot {
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
        },
        TargetValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => AssignedValueHomeKind::RuntimePointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        TargetValueOperand::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => AssignedValueHomeKind::RuntimeFrameIndexed {
            descriptor_offset: *descriptor_offset,
            index_region: *index_region,
            index_offset: *index_offset,
            index_byte_size: *index_byte_size,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        TargetValueOperand::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => AssignedValueHomeKind::RuntimeFrameBaseIndexed {
            base_byte_offset: *base_byte_offset,
            index_offset: *index_offset,
            index_byte_size: *index_byte_size,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        TargetValueOperand::FrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => AssignedValueHomeKind::RuntimeFrameFixedIndexed {
            descriptor_offset: *descriptor_offset,
            element_index: *element_index,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        TargetValueOperand::BitField { .. }
        | TargetValueOperand::Binary { .. }
        | TargetValueOperand::Convert { .. }
        | TargetValueOperand::TextEquals { .. }
        | TargetValueOperand::TextEqualsLiteral { .. }
        // A machine-indexed element read evaluates into a register like the
        // computed operands above (its address math needs no dedicated home).
        | TargetValueOperand::MachineIndexed { .. } => {
            let name = registers::scratch_register_name(
                target_operations.target.architecture,
                *next_scratch_slot,
            );
            *next_scratch_slot = next_scratch_slot.saturating_add(1);
            AssignedValueHomeKind::ScratchRegister {
                bank: AssignedRegisterBank::GeneralPurpose,
                name,
            }
        }
    }
}
