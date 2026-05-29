use omega_target_operations::TargetValueOperand;

use crate::remap;

pub(crate) fn translate_runtime_value_operand(
    operand: &omega_abstract_operations::AbstractValueOperand,
) -> TargetValueOperand {
    match operand {
        omega_abstract_operations::AbstractValueOperand::Immediate(value) => {
            TargetValueOperand::Immediate(*value)
        }
        omega_abstract_operations::AbstractValueOperand::Storage {
            region,
            byte_offset,
            byte_size,
        } => TargetValueOperand::Storage {
            region: *region,
            byte_offset: *byte_offset,
            byte_size: *byte_size,
        },
        omega_abstract_operations::AbstractValueOperand::Pointee {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
        } => TargetValueOperand::Pointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        omega_abstract_operations::AbstractValueOperand::FrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => TargetValueOperand::FrameIndexed {
            descriptor_offset: *descriptor_offset,
            index_offset: *index_offset,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        omega_abstract_operations::AbstractValueOperand::FrameBaseIndexed {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => TargetValueOperand::FrameBaseIndexed {
            base_byte_offset: *base_byte_offset,
            index_offset: *index_offset,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        omega_abstract_operations::AbstractValueOperand::FrameFixedIndexed {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            byte_size,
        } => TargetValueOperand::FrameFixedIndexed {
            descriptor_offset: *descriptor_offset,
            element_index: *element_index,
            element_byte_size: *element_byte_size,
            field_byte_offset: *field_byte_offset,
            byte_size: *byte_size,
        },
        omega_abstract_operations::AbstractValueOperand::Binary {
            left,
            operator,
            right,
        } => TargetValueOperand::Binary {
            left: remap::runtime_value_handle(*left),
            operator: *operator,
            right: remap::runtime_value_handle(*right),
        },
    }
}
