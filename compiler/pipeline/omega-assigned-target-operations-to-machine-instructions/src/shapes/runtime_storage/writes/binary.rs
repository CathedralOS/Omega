use omega_assigned_target_operations::{SelectedInstructionKind, StateGuardOperator};
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_binary_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
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
        _ => None,
    }
}

fn runtime_storage_binary_write_kind(
    _target_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite
}

fn runtime_pointee_binary_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeBinaryWrite
}

fn runtime_frame_indexed_binary_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
}

fn runtime_frame_base_indexed_binary_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedBinaryWrite
}
