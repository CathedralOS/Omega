mod binary;
mod integer;
mod string;

use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_write_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    integer::selected_integer_write_kind(kind)
        .or_else(|| binary::selected_binary_write_kind(kind))
        .or_else(|| string::selected_string_write_kind(kind))
}
