use super::super::offsets::{
    external_call_relocation_kind, runtime_text_line_read_import_call_offset,
    runtime_text_line_read_target_address_offset,
};
use super::context::InstructionRelocationContext;
use super::queries::selected_host_text_read;
use omega_calling_conventions::HostBindingMechanism;
use omega_object_file::{RelocationRecord, object_symbol_handle_by_name};
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_read_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    let Some(read) = selected_host_text_read(instruction) else {
        return;
    };
    let Some(binding) = context.input.instructions.host_binding(read.operation_key) else {
        return;
    };

    let buffer_symbol = context.data_object_symbol_handle(read.buffer);
    let target_symbol = context.storage_region_symbol_handle(read.target_region);
    context.insert_data_address_at_instruction_start(buffer_symbol);
    context.insert_data_address_at_relative_offset(
        runtime_text_line_read_target_address_offset(
            context.input.target.architecture,
            &binding.mechanism,
        ),
        target_symbol,
    );

    if let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism {
        context
            .relocation_plan
            .record_set
            .records
            .insert(RelocationRecord {
                function_symbol_handle: context.function_symbol_handle,
                selected_instruction_index: context.selected_instruction_index,
                text_offset: runtime_text_line_read_import_call_offset(
                    context.input.target.architecture,
                    context.selected_text_offset,
                ),
                byte_width: 4,
                symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol.as_ref()),
                kind: external_call_relocation_kind(context.input.target.architecture),
            });
    }
}
