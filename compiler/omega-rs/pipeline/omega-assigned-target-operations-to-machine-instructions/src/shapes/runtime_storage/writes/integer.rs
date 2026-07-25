use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_integer_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        // Write rung 2a: the place-shaped write keeps the plain integer
        // write's machine shape (the CopyPlaces precedent -- branch
        // distances see the same guarded-effect class; the layout arm's
        // re-encode is the width source of truth).
        SelectedInstructionKind::WritePlaceInteger {
            value, byte_size, ..
        } => Some(runtime_storage_integer_write_kind(0, *byte_size, *value)),
        SelectedInstructionKind::WriteStorageBitField { .. } => {
            Some(MachineInstructionKind::RuntimeStorageBitFieldWrite)
        }
        _ => None,
    }
}

fn runtime_storage_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}
