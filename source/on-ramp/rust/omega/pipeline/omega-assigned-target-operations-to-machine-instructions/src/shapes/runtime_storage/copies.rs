use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_copy_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // The Place-pair copy keeps the retired plain copy's machine shape:
        // branch distances treat it as the same guarded-effect class.
        SelectedInstructionKind::CopyPlaces { .. } => {
            Some(MachineInstructionKind::RuntimeStorageCopy)
        }
        _ => None,
    }
}
