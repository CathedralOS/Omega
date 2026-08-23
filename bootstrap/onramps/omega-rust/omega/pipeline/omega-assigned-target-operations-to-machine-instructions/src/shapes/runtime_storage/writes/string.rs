use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_string_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Text rung 2a: the place-shaped writes keep the plain string /
        // carrier machine shapes (the WritePlaceInteger precedent -- the
        // layout arm's re-encode is the width source of truth).
        SelectedInstructionKind::WritePlaceString { .. } => {
            Some(MachineInstructionKind::RuntimeMachineStringWrite)
        }
        SelectedInstructionKind::WritePlaceBoundedBuffer { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferWrite)
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferSourceAppend)
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { .. } => {
            Some(MachineInstructionKind::RuntimeMachineBoundedBufferLiteralAppend)
        }
        _ => None,
    }
}
