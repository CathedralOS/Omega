use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::{HostAbiPlan, HostCapability, HostOperation, HostOperationKey};
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use omega_target_operations::{
    InstructionOperand, InstructionOperandKind, TargetOperation, TargetOperationFunction,
    TargetOperationKind, TargetOperationPlan, TargetValueOperand,
};

use crate::host;
use crate::remap;

pub(crate) fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
) -> TargetOperationPlan {
    let mut target_operations = TargetOperationPlan::with_capacity(
        target,
        abstract_operations.functions.len(),
        abstract_operations.instructions.len(),
        abstract_operations.operands.len(),
        abstract_operations.runtime_value_operands.len(),
    );

    for (_, operand) in abstract_operations.operands.iter() {
        target_operations
            .operands
            .insert(translate_operand(operand));
    }
    for (_, operand) in abstract_operations.runtime_value_operands.iter() {
        target_operations
            .runtime_value_operands
            .insert(translate_runtime_value_operand(operand));
    }

    for (_, instruction) in abstract_operations.instructions.iter() {
        target_operations
            .instructions
            .insert(translate_instruction(host_calls, instruction));
    }

    for (_, function) in abstract_operations.functions.iter() {
        target_operations.functions.insert(TargetOperationFunction {
            symbol: std::sync::Arc::clone(&function.symbol),
            source_key: function.source_key,
            instructions: remap::instruction_span(function.instructions),
        });
    }

    host::copy_runtime_text_host_bindings(host_abi, abstract_operations, &mut target_operations);
    target_operations.boundary_edges = abstract_operations.boundary_edges.clone();
    target_operations.ownership = abstract_operations.ownership.clone();

    target_operations
}

fn translate_instruction(
    host_calls: &HostCallPlan,
    instruction: &omega_abstract_operations::AbstractOperation,
) -> TargetOperation {
    TargetOperation {
        kind: translate_instruction_kind(host_calls, instruction),
        source_key: instruction.source_key,
        source_statement: instruction.source_statement,
    }
}

fn translate_instruction_kind(
    host_calls: &HostCallPlan,
    instruction: &omega_abstract_operations::AbstractOperation,
) -> TargetOperationKind {
    match &instruction.kind {
        omega_abstract_operations::AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands,
        } => {
            let operation_key =
                host::resolve_operation_key(host_calls, instruction, *operation_ordinal);
            TargetOperationKind::HostOperation {
                operation_key,
                operands: remap::operand_span(*operands),
            }
        }
        omega_abstract_operations::AbstractOperationKind::PreparePlatformOutputHandle {
            operands,
        } => TargetOperationKind::HostOperation {
            operation_key: HostOperationKey::new(
                HostCapability::Stdout,
                HostOperation::GetStdHandle,
            ),
            operands: remap::operand_span(*operands),
        },
        omega_abstract_operations::AbstractOperationKind::WritePlatformNewline {
            use_file_api,
            operands,
        } => TargetOperationKind::HostOperation {
            operation_key: HostOperationKey::new(
                HostCapability::Stdout,
                if *use_file_api {
                    HostOperation::WriteFile
                } else {
                    HostOperation::Write
                },
            ),
            operands: remap::operand_span(*operands),
        },
        kind => TargetOperationKind::from(kind),
    }
}

fn translate_operand(
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

fn translate_runtime_value_operand(
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
