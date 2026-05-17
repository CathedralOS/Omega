use super::super::lookups::find_host_binding;
use super::super::offsets::{
    external_call_relocation_kind, external_call_relocation_offset, external_call_relocation_width,
};
use super::context::InstructionRelocationContext;
use omega_calling_conventions::HostBindingMechanism;
use omega_object::{RelocationRecord, object_symbol_handle_by_name};
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_host_operation_relocation(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) {
    let SelectedInstructionKind::HostOperation {
        operation_key,
        operands,
    } = instruction
    else {
        return;
    };
    let Some(binding) = find_host_binding(context.input, *operation_key) else {
        return;
    };
    let HostBindingMechanism::Import { symbol, .. } = &binding.mechanism else {
        return;
    };

    context.relocation_plan.records.insert(RelocationRecord {
        function_symbol: context.function.symbol.to_string(),
        selected_instruction_index: context.selected_instruction_index,
        text_offset: external_call_relocation_offset(
            context.input.target.architecture,
            context.selected_text_offset,
            context
                .input
                .instructions
                .operands
                .span(*operands)
                .unwrap_or(&[]),
        ),
        byte_width: external_call_relocation_width(context.input.target.architecture),
        symbol: symbol.clone(),
        symbol_handle: object_symbol_handle_by_name(&context.input.object, symbol),
        kind: external_call_relocation_kind(context.input.target.architecture),
    });
}
