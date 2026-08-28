use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_address_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Task #131: the place-shaped address write keeps the plain
        // storage-address machine shape (the WritePlaceInteger precedent --
        // the layout arm's re-encode is the width source of truth).
        SelectedInstructionKind::WritePlaceAddress { .. } => {
            Some(MachineInstructionKind::RuntimeStorageAddressToRuntimeFrameWrite)
        }
        _ => None,
    }
}
