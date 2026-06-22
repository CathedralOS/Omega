use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_materialize_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
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
        _ => None,
    }
}

fn runtime_text_buffer_materialize_kind(_target_offset: usize) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterialize
}

fn runtime_text_buffer_materialize_to_runtime_pointee_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimePointee
}

fn runtime_text_buffer_materialize_to_runtime_frame_indexed_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimeFrameIndexed
}
