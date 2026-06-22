use omega_assigned_target_operations::{RuntimeStorageRegion, SelectedInstructionKind};
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_integer_write_kind(
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
    _index_region: RuntimeStorageRegion,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedIntegerWrite
}
