mod addresses;
mod compare;
mod copies;
mod writes;

use omega_assigned_target_operations::SelectedInstructionKind;
use omega_machine_instructions::MachineInstructionKind;

pub(super) fn selected_runtime_storage_kind(
    kind: &SelectedInstructionKind,
) -> Option<MachineInstructionKind> {
    compare::selected_runtime_storage_compare_kind(kind)
        .or_else(|| writes::selected_runtime_storage_write_kind(kind))
        .or_else(|| addresses::selected_runtime_storage_address_kind(kind))
        .or_else(|| copies::selected_runtime_storage_copy_kind(kind))
}
