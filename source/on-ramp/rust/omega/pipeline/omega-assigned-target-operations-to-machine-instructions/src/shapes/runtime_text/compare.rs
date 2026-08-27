use omega_assigned_target_operations::{SelectedInstructionKind, StateGuardOperator};
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_compare_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    match kind {
        SelectedInstructionKind::CompareRuntimeTextLiteral { .. } => {
            Some(runtime_text_literal_compare_kind())
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => Some(runtime_text_storage_compare_kind(*source_offset, *operator)),
        _ => None,
    }
}

fn runtime_text_literal_compare_kind() -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralCompare
}

fn runtime_text_storage_compare_kind(
    _source_offset: usize,
    _operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStorageCompare
}
