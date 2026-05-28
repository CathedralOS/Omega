use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => Some(runtime_machine_integer_write_kind(
            *byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            byte_offset,
            byte_size,
            value,
            ..
        } => Some(runtime_storage_integer_write_kind(
            *byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimePointeeInteger {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            value,
        } => Some(runtime_pointee_integer_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_offset,
            byte_size,
            operator,
            ..
        } => Some(runtime_storage_binary_write_kind(
            *target_offset,
            *byte_size,
            *operator,
        )),
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            byte_size,
            operator,
            ..
        } => Some(runtime_pointee_binary_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_size,
            *operator,
        )),
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => Some(runtime_frame_indexed_integer_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => Some(runtime_frame_base_indexed_integer_write_kind(
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            value,
        } => Some(runtime_machine_indexed_integer_write_kind(
            *base_byte_offset,
            *index_region,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *value,
        )),
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            operator,
            ..
        } => Some(runtime_frame_indexed_binary_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *operator,
        )),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_size,
            operator,
            ..
        } => Some(runtime_frame_base_indexed_binary_write_kind(
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_size,
            *operator,
        )),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => Some(runtime_machine_string_write_kind(
            *byte_offset,
            *byte_length,
        )),
        SelectedInstructionKind::WriteRuntimePointeeString {
            pointer_byte_offset,
            field_byte_offset,
            byte_length,
            ..
        } => Some(runtime_pointee_string_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_length,
        )),
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => Some(runtime_frame_indexed_string_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        )),
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_length,
            ..
        } => Some(runtime_machine_indexed_string_write_kind(
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_length,
        )),
        _ => None,
    }
}

fn runtime_machine_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

fn runtime_storage_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

fn runtime_pointee_integer_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeIntegerWrite
}

fn runtime_storage_binary_write_kind(
    _target_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite
}

fn runtime_pointee_binary_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeBinaryWrite
}

fn runtime_frame_indexed_integer_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedIntegerWrite
}

fn runtime_frame_base_indexed_integer_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedIntegerWrite
}

fn runtime_machine_indexed_integer_write_kind(
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

fn runtime_frame_indexed_binary_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
}

fn runtime_frame_base_indexed_binary_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedBinaryWrite
}

fn runtime_machine_string_write_kind(
    _byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineStringWrite
}

fn runtime_pointee_string_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeStringWrite
}

fn runtime_frame_indexed_string_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedStringWrite
}

fn runtime_machine_indexed_string_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedStringWrite
}
