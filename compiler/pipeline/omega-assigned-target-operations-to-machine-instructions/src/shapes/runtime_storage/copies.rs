use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_copy_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_kind(
            *source_offset,
            *target_offset,
            *byte_count,
        )),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_to_runtime_frame_indexed_kind(
            *source_offset,
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *byte_count,
        )),
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        }
        | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_from_runtime_frame_indexed_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        )),
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        }
        | SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_from_runtime_frame_fixed_indexed_kind(
            *descriptor_offset,
            *element_index,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        )),
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_from_runtime_machine_indexed_kind(
            *base_byte_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
            *target_offset,
            *byte_count,
        )),
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            byte_count,
            ..
        } => Some(runtime_storage_copy_to_runtime_pointee_kind(
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
            *byte_count,
        )),
        _ => None,
    }
}

fn runtime_storage_copy_kind(
    _source_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopy
}

fn runtime_storage_copy_to_runtime_frame_indexed_kind(
    _source_offset: usize,
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimeFrameIndexed
}

fn runtime_storage_copy_from_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameIndexed
}

fn runtime_storage_copy_from_runtime_frame_fixed_indexed_kind(
    _descriptor_offset: usize,
    _element_index: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameFixedIndexed
}

fn runtime_storage_copy_from_runtime_machine_indexed_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _target_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyFromRuntimeMachineIndexed
}

fn runtime_storage_copy_to_runtime_pointee_kind(
    _source_offset: usize,
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_count: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCopyToRuntimePointee
}
