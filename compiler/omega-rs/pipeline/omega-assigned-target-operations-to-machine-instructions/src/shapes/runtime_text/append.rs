use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_append_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => Some(runtime_text_stored_suffix_append_kind(
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        )),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => Some(runtime_text_stored_place_append_kind(
            *source_offset,
            *target_offset,
        )),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
            source_offset,
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => Some(runtime_text_stored_place_append_to_runtime_pointee_kind(
            *source_offset,
            *pointer_byte_offset,
            *field_byte_offset,
        )),
        SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
            source_offset,
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => Some(
            runtime_text_stored_place_append_to_runtime_frame_indexed_kind(
                *source_offset,
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
            ),
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral { target_offset, .. } => {
            Some(runtime_text_literal_append_kind(*target_offset))
        }
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => Some(runtime_text_literal_append_to_runtime_pointee_kind(
            *pointer_byte_offset,
            *field_byte_offset,
        )),
        SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => Some(runtime_text_literal_append_to_runtime_frame_indexed_kind(
            *descriptor_offset,
            *index_offset,
            *element_byte_size,
            *field_byte_offset,
        )),
        _ => None,
    }
}

fn runtime_text_stored_suffix_append_kind(
    _buffer_offset: usize,
    _source_offset: usize,
    _target_offset: usize,
    _length_delta: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredSuffixAppend
}

fn runtime_text_stored_place_append_kind(
    _source_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppend
}

fn runtime_text_stored_place_append_to_runtime_pointee_kind(
    _source_offset: usize,
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimePointee
}

fn runtime_text_stored_place_append_to_runtime_frame_indexed_kind(
    _source_offset: usize,
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed
}

fn runtime_text_literal_append_kind(_target_offset: usize) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppend
}

fn runtime_text_literal_append_to_runtime_pointee_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppendToRuntimePointee
}

fn runtime_text_literal_append_to_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppendToRuntimeFrameIndexed
}
