use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_append_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Task #132: the place-shaped survivors keep the plain append
        // machine shapes.
        SelectedInstructionKind::AppendTextStoredToPlace { .. } => {
            Some(MachineInstructionKind::RuntimeTextStoredPlaceAppend)
        }
        SelectedInstructionKind::AppendTextLiteralToPlace { .. } => {
            Some(MachineInstructionKind::RuntimeTextLiteralAppend)
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
