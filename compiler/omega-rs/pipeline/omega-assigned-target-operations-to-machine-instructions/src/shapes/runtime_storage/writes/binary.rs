use omega_assigned_target_operations::{
    RuntimeStorageRegion, SelectedInstructionKind, StateGuardOperator,
};
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_binary_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::WritePlaceBinary {
            byte_size, operator, ..
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

fn runtime_pointee_binary_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeBinaryWrite
}

fn runtime_frame_indexed_binary_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
}

fn runtime_frame_base_indexed_binary_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedBinaryWrite
}

fn runtime_machine_indexed_binary_write_kind(
    _base_byte_offset: usize,
    _index_region: RuntimeStorageRegion,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedBinaryWrite
}
