use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_compare_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Task #131: the place-shaped compares keep the plain compare
        // machine shapes (the WritePlaceInteger precedent -- the layout
        // arm's re-encode is the width source of truth).
        SelectedInstructionKind::ComparePlaces { .. } => {
            Some(MachineInstructionKind::RuntimeStorageCompare)
        }
        SelectedInstructionKind::ComparePlaceValue { .. } => {
            Some(MachineInstructionKind::RuntimeStorageValueCompare)
        }
        SelectedInstructionKind::CompareRuntimeValues { .. } => Some(MachineInstructionKind::NoOp),
        _ => None,
    }
}
