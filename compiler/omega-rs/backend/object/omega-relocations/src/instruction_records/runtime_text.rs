use super::byte_io;
use super::context::InstructionRelocationContext;
use super::runtime_text_append;
use super::runtime_text_compare;
use super::runtime_text_materialize;
use super::runtime_text_read;
use super::runtime_text_write;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    if runtime_text_append::collect_runtime_text_append_relocations(context, instruction) {
        return;
    }
    if runtime_text_materialize::collect_runtime_text_materialize_relocations(context, instruction)
    {
        return;
    }
    if runtime_text_compare::collect_runtime_text_compare_relocations(context, instruction) {
        return;
    }
    if runtime_text_write::collect_runtime_text_write_relocations(context, instruction) {
        return;
    }
    if byte_io::collect_runtime_byte_io_relocations(context, instruction) {
        return;
    }

    match instruction {
        SelectedInstructionKind::ReadRuntimeTextLine { .. } => {
            runtime_text_read::collect_runtime_text_read_relocations(context, instruction)
        }
        _ => {}
    }
}
