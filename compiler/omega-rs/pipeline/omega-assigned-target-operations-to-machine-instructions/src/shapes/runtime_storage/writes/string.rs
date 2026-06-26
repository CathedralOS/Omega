use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_string_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => Some(runtime_machine_string_write_kind(
            *byte_offset,
            *byte_length,
        )),
        SelectedInstructionKind::WriteRuntimeMachineBoundedBuffer { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferWrite)
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferSource { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferSourceAppend)
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferLiteral { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferLiteralAppend)
        }
        SelectedInstructionKind::WriteRuntimeFrameString {
            byte_offset,
            byte_length,
            ..
        } => Some(runtime_frame_string_write_kind(*byte_offset, *byte_length)),
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

fn runtime_machine_string_write_kind(
    _byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineStringWrite
}

fn runtime_frame_string_write_kind(
    _byte_offset: usize,
    _byte_length: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameStringWrite
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
