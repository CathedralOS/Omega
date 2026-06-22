use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_compare_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => Some(runtime_storage_compare_kind(
            *left_offset,
            *right_offset,
            *byte_size,
            *operator,
        )),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => Some(runtime_storage_value_compare_kind(
            *byte_offset,
            *byte_size,
            *expected_value,
            *operator,
        )),
        SelectedInstructionKind::CompareRuntimeValues { .. } => Some(MachineInstructionKind::NoOp),
        _ => None,
    }
}

fn runtime_storage_compare_kind(
    _left_offset: usize,
    _right_offset: usize,
    _byte_size: usize,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageCompare
}

fn runtime_storage_value_compare_kind(
    _byte_offset: usize,
    _byte_size: usize,
    _expected_value: i64,
    _operator: omega_assigned_target_operations::StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeStorageValueCompare
}
