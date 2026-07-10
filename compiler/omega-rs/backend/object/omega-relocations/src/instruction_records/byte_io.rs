use super::super::offsets::external_call_relocation_kind;
use super::context::InstructionRelocationContext;
use omega_calling_conventions::HostBindingMechanism;
use omega_object_file::{RelocationRecord, object_symbol_handle_by_name};
use omega_target_operations::{RuntimeTextReadSource, SelectedInstructionKind};

/// Relocations for the console byte-op composites: the `adrp`+`add` pair at
/// instruction start binds the addressed storage (the ByteRead target region
/// for reads; the source region OR the staged 1-byte literal object for
/// writes), and an Import binding adds the `bl` call record at the encoder's
/// fixed offset. Syscall bindings need no call record.
pub(super) fn collect_runtime_byte_io_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::ReadRuntimeByte {
            target_region,
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        } => {
            let region_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(region_symbol);
            insert_import_call_record(
                context,
                *operation_key,
                omega_instruction_selection::runtime_byte_read_import_call_offset(
                    context.input.target.architecture,
                ),
            );
            true
        }
        SelectedInstructionKind::WriteRuntimeByte {
            source_region,
            literal,
            source_is_place,
            source: RuntimeTextReadSource::HostOperation { operation_key },
            ..
        } => {
            let address_symbol = if *source_is_place {
                context.storage_region_symbol_handle(*source_region)
            } else {
                context.data_object_symbol_handle(*literal)
            };
            context.insert_data_address_at_instruction_start(address_symbol);
            insert_import_call_record(
                context,
                *operation_key,
                omega_instruction_selection::runtime_byte_write_import_call_offset(
                    context.input.target.architecture,
                ),
            );
            true
        }
        _ => false,
    }
}

fn insert_import_call_record(
    context: &mut InstructionRelocationContext<'_, '_>,
    operation_key: omega_calling_conventions::HostOperationKey,
    call_offset_in_instruction: usize,
) {
    let Some(binding) = context.input.instructions.host_binding(operation_key) else {
        return;
    };
    let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism else {
        return;
    };

    context
        .relocation_plan
        .record_set
        .records
        .insert(RelocationRecord {
            function_symbol_handle: context.function_symbol_handle,
            selected_instruction_index: context.selected_instruction_index,
            text_offset: context.selected_text_offset + call_offset_in_instruction,
            byte_width: 4,
            symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol.as_ref()),
            kind: external_call_relocation_kind(context.input.target.architecture),
        });
}
