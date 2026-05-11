use omega_machine_program::{MachineInstructionKind, MachineRuntimeValueOperand};
use omega_target_operations::{RuntimeValueOperand, StateGuardOperator};

pub(super) fn runtime_storage_compare_kind(
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCompare {
        left_offset,
        right_offset,
        byte_size,
        operator,
    }
}

pub(super) fn runtime_storage_value_compare_kind(
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageValueCompare {
        byte_offset,
        byte_size,
        expected_value,
        operator,
    }
}

pub(super) fn runtime_machine_integer_write_kind(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite {
        byte_offset,
        byte_size,
        value,
    }
}

pub(super) fn runtime_storage_integer_write_kind(
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite {
        byte_offset,
        byte_size,
        value,
    }
}

pub(super) fn runtime_storage_binary_write_kind(
    target_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperand,
    operator: StateGuardOperator,
    right: RuntimeValueOperand,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite {
        target_offset,
        byte_size,
        left: lower_runtime_value_operand(left),
        operator,
        right: lower_runtime_value_operand(right),
    }
}

pub(super) fn runtime_frame_indexed_integer_write_kind(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedIntegerWrite {
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        value,
    }
}

pub(super) fn runtime_frame_indexed_binary_write_kind(
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_size: usize,
    left: RuntimeValueOperand,
    operator: StateGuardOperator,
    right: RuntimeValueOperand,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite {
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        left: lower_runtime_value_operand(left),
        operator,
        right: lower_runtime_value_operand(right),
    }
}

pub(super) fn runtime_machine_string_write_kind(
    byte_offset: usize,
    byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineStringWrite {
        byte_offset,
        byte_length,
    }
}

pub(super) fn runtime_storage_copy_kind(
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopy {
        source_offset,
        target_offset,
        byte_count,
    }
}

pub(super) fn runtime_storage_copy_to_runtime_frame_indexed_kind(
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimeFrameIndexed {
        source_offset,
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_count,
    }
}

fn lower_runtime_value_operand(operand: RuntimeValueOperand) -> MachineRuntimeValueOperand {
    match operand {
        RuntimeValueOperand::Immediate(value) => MachineRuntimeValueOperand::Immediate(value),
        RuntimeValueOperand::Storage {
            byte_offset,
            byte_size,
            ..
        } => {
            MachineRuntimeValueOperand::Storage {
                byte_offset,
                byte_size,
            }
        }
    }
}
