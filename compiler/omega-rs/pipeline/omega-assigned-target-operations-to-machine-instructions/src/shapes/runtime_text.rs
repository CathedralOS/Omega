mod append;
mod compare;
mod materialize;
mod read;
mod write;

use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_text_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    compare::selected_compare_kind(kind)
        .or_else(|| write::selected_write_kind(kind))
        .or_else(|| append::selected_append_kind(kind))
        .or_else(|| materialize::selected_materialize_kind(kind))
        .or_else(|| read::selected_read_kind(kind))
}
