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

pub(super) fn runtime_pointee_integer_write_kind(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeIntegerWrite {
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        value,
    }
}

pub(super) fn runtime_storage_binary_write_kind(
    target_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite {
        target_offset,
        byte_size,
        operator,
    }
}

pub(super) fn runtime_pointee_binary_write_kind(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeBinaryWrite {
        pointer_byte_offset,
        field_byte_offset,
        byte_size,
        operator,
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
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite {
        descriptor_offset,
        index_offset,
        element_byte_size,
        field_byte_offset,
        byte_size,
        operator,
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

pub(super) fn runtime_pointee_string_write_kind(
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeStringWrite {
        pointer_byte_offset,
        field_byte_offset,
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

pub(super) fn runtime_storage_copy_to_runtime_pointee_kind(
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimePointee {
        source_offset,
        pointer_byte_offset,
        field_byte_offset,
        byte_count,
    }
}
