use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_materialize_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Task #132: the place-shaped survivor keeps the plain materialize
        // machine shape (the layout arm's re-encode is the width authority).
        SelectedInstructionKind::MaterializeTextBufferToPlace { .. } => {
            Some(MachineInstructionKind::RuntimeTextBufferMaterialize)
        }
        _ => None,
    }
}
