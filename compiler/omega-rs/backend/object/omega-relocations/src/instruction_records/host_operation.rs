use super::super::data_addresses::collect_data_address_relocations;
use super::super::lookups::find_host_binding;
use super::super::offsets::{
    external_call_relocation_kind, external_call_relocation_offset, external_call_relocation_width,
};
use super::context::InstructionRelocationContext;
use super::queries::selected_host_operation;
use omega_calling_conventions::HostBindingMechanism;
use omega_core::arena::HandleSpan;
use omega_object_file::{RelocationRecord, object_symbol_handle_by_name};
use omega_target_operations::{InstructionOperand, SelectedInstructionKind};

pub(super) fn collect_host_operation_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    let Some((operation_key, operands)) = selected_host_operation(instruction) else {
        return false;
    };

    collect_data_address_relocations(
        context.input,
        context.function_symbol_handle,
        context.selected_instruction_index,
        Some(operation_key),
        operands,
        context.selected_text_offset,
        context.relocation_plan,
    );
    collect_host_operation_call_relocation(context, operation_key, operands);
    true
}

fn collect_host_operation_call_relocation(
    context: &mut InstructionRelocationContext<'_, '_>,
    operation_key: omega_calling_conventions::HostOperationKey,
    operands: HandleSpan<InstructionOperand>,
) {
    let Some(binding) = find_host_binding(context.input, operation_key) else {
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
            text_offset: external_call_relocation_offset(
                context.input.target.architecture,
                operation_key,
                context.selected_text_offset,
                context
                    .input
                    .assigned_target_operations
                    .instruction_operands(operands)
                    .unwrap_or(&[]),
            ),
            byte_width: external_call_relocation_width(context.input.target.architecture),
            symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol.as_ref()),
            kind: external_call_relocation_kind(context.input.target.architecture),
        });
}
