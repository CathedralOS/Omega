use omega_assigned_target_operations::{
    RuntimeStorageRegion, SelectedInstructionKind, StateGuardOperator,
};
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_binary_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WritePlaceBinary {
            byte_size,
            operator,
            ..
        } => Some(runtime_storage_binary_write_kind(0, *byte_size, *operator)),
        SelectedInstructionKind::WriteRuntimeStorageConvert { .. } => {
            Some(MachineInstructionKind::RuntimeStorageConvert)
        }
        SelectedInstructionKind::AtomicFetchAdd { .. } => {
            Some(MachineInstructionKind::AtomicFetchAdd)
        }
        SelectedInstructionKind::AtomicCompareExchange { .. } => {
            Some(MachineInstructionKind::AtomicCompareExchange)
        }
        _ => None,
    }
}

fn runtime_storage_binary_write_kind(
    _target_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageBinaryWrite
}
