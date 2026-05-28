use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_text_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::CompareRuntimeTextLiteral { .. } => {
            Some(runtime_text_literal_compare_kind())
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => Some(runtime_text_storage_compare_kind(*source_offset, *operator)),
        SelectedInstructionKind::WriteRuntimeTextLiteral { .. } => {
            Some(runtime_text_literal_write_kind())
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { byte_offset, .. } => {
            Some(runtime_text_literal_segment_write_kind(*byte_offset))
        }
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
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            Some(runtime_text_buffer_materialize_kind(*target_offset))
        }
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee {
            pointer_byte_offset,
            field_byte_offset,
            ..
        } => Some(runtime_text_buffer_materialize_to_runtime_pointee_kind(
            *pointer_byte_offset,
            *field_byte_offset,
        )),
        SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
            descriptor_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => Some(
            runtime_text_buffer_materialize_to_runtime_frame_indexed_kind(
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
            ),
        ),
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
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            ..
        } => Some(runtime_text_line_read_kind(*target_offset, *byte_capacity)),
        _ => None,
    }
}

pub(super) fn runtime_text_literal_compare_kind() -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralCompare
}

pub(super) fn runtime_text_storage_compare_kind(
    _source_offset: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStorageCompare
}

pub(super) fn runtime_text_literal_write_kind() -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralWrite
}

pub(super) fn runtime_text_literal_segment_write_kind(
    _byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralSegmentWrite
}

pub(super) fn runtime_text_stored_suffix_append_kind(
    _buffer_offset: usize,
    _source_offset: usize,
    _target_offset: usize,
    _length_delta: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredSuffixAppend
}

pub(super) fn runtime_text_buffer_materialize_kind(
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterialize
}

pub(super) fn runtime_text_buffer_materialize_to_runtime_pointee_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimePointee
}

pub(super) fn runtime_text_buffer_materialize_to_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimeFrameIndexed
}

pub(super) fn runtime_text_stored_place_append_kind(
    _source_offset: usize,
    _target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppend
}

pub(super) fn runtime_text_stored_place_append_to_runtime_pointee_kind(
    _source_offset: usize,
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimePointee
}

pub(super) fn runtime_text_stored_place_append_to_runtime_frame_indexed_kind(
    _source_offset: usize,
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed
}

pub(super) fn runtime_text_literal_append_kind(_target_offset: usize) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppend
}

pub(super) fn runtime_text_literal_append_to_runtime_pointee_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppendToRuntimePointee
}

pub(super) fn runtime_text_literal_append_to_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppendToRuntimeFrameIndexed
}

pub(super) fn runtime_text_line_read_kind(
    _target_offset: usize,
    _byte_capacity: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLineRead
}
