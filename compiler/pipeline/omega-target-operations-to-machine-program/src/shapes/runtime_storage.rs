use omega_machine_program::MachineInstructionKind;
use omega_target_operations::StateGuardOperator;

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
