use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_address_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_offset,
            target_offset,
            ..
        } => Some(runtime_storage_address_to_runtime_frame_write_kind(
            *source_offset,
            *target_offset,
        )),
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset,
            ..
        } => Some(runtime_pointee_address_to_runtime_frame_write_kind(
            *pointer_byte_offset,
            *field_byte_offset,
            *target_offset,
        )),
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => Some(runtime_frame_indexed_address_to_runtime_frame_write_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
        )),
        SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => Some(
            runtime_frame_fixed_indexed_address_to_runtime_frame_write_kind(
                *descriptor_offset,
                *element_index,
                *element_byte_size,
                *field_byte_offset,
                *target_offset,
            ),
        ),
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
        } => Some(
            runtime_frame_base_indexed_address_to_runtime_frame_write_kind(
                *base_byte_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
                *target_offset,
            ),
        ),
        _ => None,
    }
}

fn runtime_storage_address_to_runtime_frame_write_kind(
    _source_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageAddressToRuntimeFrameWrite
}

fn runtime_pointee_address_to_runtime_frame_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeAddressToRuntimeFrameWrite
}

fn runtime_frame_indexed_address_to_runtime_frame_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedAddressToRuntimeFrameWrite
}

fn runtime_frame_fixed_indexed_address_to_runtime_frame_write_kind(
    _descriptor_offset: usize,
    _element_index: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameFixedIndexedAddressToRuntimeFrameWrite
}

fn runtime_frame_base_indexed_address_to_runtime_frame_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite
}
