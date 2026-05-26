use omega_machine_instructions::MachineInstructionKind;

pub(super) fn runtime_storage_compare_kind(
    _left_offset: usize,
    _right_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCompare
}

pub(super) fn runtime_storage_value_compare_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _expected_value: i64,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageValueCompare
}

pub(super) fn runtime_machine_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

pub(super) fn runtime_storage_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

pub(super) fn runtime_pointee_integer_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeIntegerWrite
}

pub(super) fn runtime_storage_binary_write_kind(
    _target_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite
}

pub(super) fn runtime_pointee_binary_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeBinaryWrite
}

pub(super) fn runtime_frame_indexed_integer_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedIntegerWrite
}

pub(super) fn runtime_frame_base_indexed_integer_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedIntegerWrite
}

pub(super) fn runtime_machine_indexed_integer_write_kind(
    _base_byte_offset: usize,
    _index_region: omega_assigned_target_operations::RuntimeStorageRegion,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedIntegerWrite
}

pub(super) fn runtime_frame_indexed_binary_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
}

pub(super) fn runtime_machine_string_write_kind(
    _byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineStringWrite
}

pub(super) fn runtime_pointee_string_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeStringWrite
}

pub(super) fn runtime_frame_indexed_string_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedStringWrite
}

pub(super) fn runtime_machine_indexed_string_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedStringWrite
}

pub(super) fn runtime_storage_address_to_runtime_frame_write_kind(
    _source_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageAddressToRuntimeFrameWrite
}

pub(super) fn runtime_pointee_address_to_runtime_frame_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeAddressToRuntimeFrameWrite
}

pub(super) fn runtime_frame_indexed_address_to_runtime_frame_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedAddressToRuntimeFrameWrite
}

pub(super) fn runtime_storage_copy_kind(
    _source_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopy
}

pub(super) fn runtime_storage_copy_to_runtime_frame_indexed_kind(
    _source_offset: usize,
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimeFrameIndexed
}

pub(super) fn runtime_storage_copy_from_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameIndexed
}

pub(super) fn runtime_storage_copy_from_runtime_frame_fixed_indexed_kind(
    _descriptor_offset: usize,
    _element_index: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameFixedIndexed
}

pub(super) fn runtime_storage_copy_from_runtime_machine_indexed_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeMachineIndexed
}

pub(super) fn runtime_storage_copy_to_runtime_pointee_kind(
    _source_offset: usize,
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimePointee
}
