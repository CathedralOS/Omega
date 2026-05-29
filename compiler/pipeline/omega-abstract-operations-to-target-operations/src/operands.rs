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
        } => InstructionOperandKind::RuntimeStringPointer {
            region: *region,
            byte_offset: *byte_offset,
        },
        omega_abstract_operations::InstructionOperandKind::RuntimeStringLength {
            region,
            byte_offset,
        } => InstructionOperandKind::RuntimeStringLength {
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
