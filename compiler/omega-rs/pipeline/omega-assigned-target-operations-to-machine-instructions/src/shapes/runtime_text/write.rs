use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WriteRuntimeTextLiteral { .. } => {
            Some(runtime_text_literal_write_kind())
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment { byte_offset, .. } => {
            Some(runtime_text_literal_segment_write_kind(*byte_offset))
        }
        _ => None,
    }
}

fn runtime_text_literal_write_kind() -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralWrite
}

fn runtime_text_literal_segment_write_kind(_byte_offset: usize) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralSegmentWrite
}
