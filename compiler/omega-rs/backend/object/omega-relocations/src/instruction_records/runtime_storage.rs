use super::context::InstructionRelocationContext;
use super::runtime_storage_addresses;
use super::runtime_storage_compares;
use super::runtime_storage_copies;
use super::runtime_storage_strings;
use super::runtime_storage_writes;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    if runtime_storage_strings::collect_runtime_storage_string_relocations(context, instruction) {
        return true;
    }
    if runtime_storage_copies::collect_runtime_storage_copy_relocations(context, instruction) {
        return true;
    }
    if runtime_storage_addresses::collect_runtime_storage_address_relocations(context, instruction)
    {
        return true;
    }
    if runtime_storage_compares::collect_runtime_storage_compare_relocations(context, instruction) {
        return true;
    }
    runtime_storage_writes::collect_runtime_storage_write_relocations(context, instruction)
}
