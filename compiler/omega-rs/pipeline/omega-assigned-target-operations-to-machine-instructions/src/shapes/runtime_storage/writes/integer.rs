use omega_assigned_target_operations::{RuntimeStorageRegion, SelectedInstructionKind};
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
        _ => None,
    }
}

fn runtime_machine_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

fn runtime_storage_integer_write_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIntegerWrite
}

fn runtime_pointee_integer_write_kind(
    _pointer_byte_offset: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimePointeeIntegerWrite
}

fn runtime_frame_indexed_integer_write_kind(
    _descriptor_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameIndexedIntegerWrite
}

fn runtime_frame_base_indexed_integer_write_kind(
    _base_byte_offset: usize,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeFrameBaseIndexedIntegerWrite
}

fn runtime_machine_indexed_integer_write_kind(
    _base_byte_offset: usize,
    _index_region: RuntimeStorageRegion,
    _index_offset: usize,
    _element_byte_size: usize,
    _field_byte_offset: usize,
    _byte_size: usize,
    _value: i64,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeMachineIndexedIntegerWrite
}
