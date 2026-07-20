use omega_target_operations::{InstructionOperand, InstructionOperandKind};

pub(crate) fn translate_operand(
    operand: &omega_abstract_operations::InstructionOperand,
) -> InstructionOperand {
    InstructionOperand {
        kind: translate_operand_kind(&operand.kind),
    }
}

fn translate_operand_kind(
    kind: &omega_abstract_operations::InstructionOperandKind,
) -> InstructionOperandKind {
    match kind {
        omega_abstract_operations::InstructionOperandKind::DataAddress { data } => {
            InstructionOperandKind::DataAddress {
                data: omega_target_operations::target_data_handle_from_abstract(*data),
            }
        }
        omega_abstract_operations::InstructionOperandKind::RuntimeStringPointer {
            region,
            byte_offset,
            is_bounded_buffer,
        } => InstructionOperandKind::RuntimeStringPointer {
            region: *region,
            byte_offset: *byte_offset,
            is_bounded_buffer: *is_bounded_buffer,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeStringLength {
            region,
            byte_offset,
            is_bounded_buffer,
        } => InstructionOperandKind::RuntimeStringLength {
            region: *region,
            byte_offset: *byte_offset,
            is_bounded_buffer: *is_bounded_buffer,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimePointeeStringPointer {
            region,
            byte_offset,
        } => InstructionOperandKind::RuntimePointeeStringPointer {
            region: *region,
            byte_offset: *byte_offset,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimePointeeStringLength {
            region,
            byte_offset,
        } => InstructionOperandKind::RuntimePointeeStringLength {
            region: *region,
            byte_offset: *byte_offset,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeScalarInteger {
            region,
            byte_offset,
            byte_count,
        } => InstructionOperandKind::RuntimeScalarInteger {
            region: *region,
            byte_offset: *byte_offset,
            byte_count: *byte_count,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeScalarFloat {
            region,
            byte_offset,
            byte_count,
        } => InstructionOperandKind::RuntimeScalarFloat {
            region: *region,
            byte_offset: *byte_offset,
            byte_count: *byte_count,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeHomogeneousFloatAggregate {
            region,
            byte_offset,
            member_byte_count,
            members,
        } => InstructionOperandKind::RuntimeHomogeneousFloatAggregate {
            region: *region,
            byte_offset: *byte_offset,
            member_byte_count: *member_byte_count,
            members: *members,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeSystemVAggregate {
            region,
            byte_offset,
            byte_count,
            alignment,
            sse_eightbytes,
        } => InstructionOperandKind::RuntimeSystemVAggregate {
            region: *region,
            byte_offset: *byte_offset,
            byte_count: *byte_count,
            alignment: *alignment,
            sse_eightbytes: *sse_eightbytes,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeSmallAggregate {
            region,
            byte_offset,
            byte_count,
            alignment,
        } => InstructionOperandKind::RuntimeSmallAggregate {
            region: *region,
            byte_offset: *byte_offset,
            byte_count: *byte_count,
            alignment: *alignment,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeLargeAggregate {
            region,
            byte_offset,
            byte_count,
            alignment,
        } => InstructionOperandKind::RuntimeLargeAggregate {
            region: *region,
            byte_offset: *byte_offset,
            byte_count: *byte_count,
            alignment: *alignment,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeStorageAddress {
            region,
            byte_offset,
        } => InstructionOperandKind::RuntimeStorageAddress {
            region: *region,
            byte_offset: *byte_offset,
        },
        omega_abstract_operations::InstructionOperandKind::ImmediateInteger(value) => {
            InstructionOperandKind::ImmediateInteger(*value)
        }
        omega_abstract_operations::InstructionOperandKind::ByteLength(value) => {
            InstructionOperandKind::ByteLength(*value)
        }
    }
}
