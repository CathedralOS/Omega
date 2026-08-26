use super::super::lookups::find_host_binding;
use super::super::offsets::{
    external_call_relocation_kind, runtime_text_line_read_get_std_handle_call_offset,
    runtime_text_line_read_import_call_offset, runtime_text_line_read_target_address_offset,
};
use super::context::InstructionRelocationContext;
use super::queries::selected_host_text_read;
use omega_calling_conventions::{
    HostBindingMechanism, HostImportLocator, HostOperation, HostOperationKey,
};
use omega_object_file::{RelocationRecord, object_symbol_handle_by_name};
use omega_target::Architecture;
use omega_target_operations::RuntimeTextReadTarget;
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

    if read.target != RuntimeTextReadTarget::StringDescriptor {
        // Inline carrier/array: relocate the read base to the target's own
        // region. The encoder applies the shape-specific field offset.
        let region_symbol = context.storage_region_symbol_handle(read.target_region);
        context.insert_data_address_at_instruction_start(region_symbol);
    } else {
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
    }

    let HostBindingMechanism::Import {
        locator: HostImportLocator::StringBackedBootstrap { symbol, .. },
    } = &binding.mechanism
    else {
        return;
    };
    let external_kind = external_call_relocation_kind(context.input.target.architecture);

    // The x86_64 line read first calls GetStdHandle(STD_INPUT_HANDLE) to obtain
    // the stdin handle, then calls ReadFile. Emit a call relocation for the
    // GetStdHandle import too (aarch64 needs no separate handle call).
    if context.input.target.architecture == Architecture::X86_64 {
        let get_std_handle_key =
            HostOperationKey::new(read.operation_key.capability, HostOperation::GetStdHandle);
        if let Some(handle_binding) = find_host_binding(context.input, get_std_handle_key)
            && let HostBindingMechanism::Import {
                locator:
                    HostImportLocator::StringBackedBootstrap {
                        symbol: handle_symbol,
                        ..
                    },
            } = &handle_binding.mechanism
        {
            context
                .relocation_plan
                .record_set
                .records
                .insert(RelocationRecord {
                    origin: omega_object_file::RelocationOrigin::Instruction {
                        function_symbol_handle: context.function_symbol_handle,
                        selected_instruction_index: context.selected_instruction_index,
                    },
                    section: omega_object_file::SectionKind::Text,
                    offset: runtime_text_line_read_get_std_handle_call_offset(
                        context.input.target.architecture,
                        context.selected_text_offset,
                        read.target,
                    ),
                    byte_width: 4,
                    symbol_handle: object_symbol_handle_by_name(
                        &context.input.object,
                        handle_symbol.as_ref(),
                    ),
                    addend: 0,
                    kind: external_kind,
                });
        }
    }

    context
        .relocation_plan
        .record_set
        .records
        .insert(RelocationRecord {
            origin: omega_object_file::RelocationOrigin::Instruction {
                function_symbol_handle: context.function_symbol_handle,
                selected_instruction_index: context.selected_instruction_index,
            },
            section: omega_object_file::SectionKind::Text,
            offset: runtime_text_line_read_import_call_offset(
                context.input.target.architecture,
                context.selected_text_offset,
                read.target,
                read.target_offset,
            ),
            byte_width: 4,
            symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol.as_ref()),
            addend: 0,
            kind: external_kind,
        });
}
