use omega_machine_program::MachineInstructionKind;

pub(super) fn runtime_text_literal_compare_kind() -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralCompare
}

pub(super) fn runtime_text_storage_compare_kind(
    _source_offset: usize,
    _operator: omega_target_operations::StateGuardOperator,
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
