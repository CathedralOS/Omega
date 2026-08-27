use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_read_kind(kind: &SelectedInstructionKind) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            ..
        } => Some(runtime_text_line_read_kind(*target_offset, *byte_capacity)),
        SelectedInstructionKind::ReadRuntimeByte { .. } => {
            Some(MachineInstructionKind::RuntimeByteRead)
        }
        SelectedInstructionKind::WriteRuntimeByte { .. } => {
            Some(MachineInstructionKind::RuntimeByteWrite)
        }
        _ => None,
    }
}

fn runtime_text_line_read_kind(
    _target_offset: usize,
    _byte_capacity: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLineRead
}
