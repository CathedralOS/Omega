use crate::RelocationPlanningInput;
use crate::data_address_records::insert_data_address_relocations;
use crate::lookups::find_host_binding;
use crate::offsets::FieldModelCallShape;
use crate::offsets::data_address_relocation_offset;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_object_file::{
    ObjectSymbolHandle, RelocationPlan, object_symbol_handle_by_name, storage_region_symbol_name,
};
use omega_target_operations::InstructionOperandLike;

pub(super) fn collect_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operation_key: Option<HostOperationKey>,
    operands: omega_core::arena::HandleSpan<omega_target_operations::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = input
        .assigned_target_operations
        .instruction_operands(operands)
    else {
        return;
    };

    // A Linux syscall host call lays its arguments out differently than a win32
    // import call, so the data-address fixup offset must use the syscall layout.
    let is_syscall = operation_key
        .and_then(|key| find_host_binding(input, key))
        .is_some_and(|binding| matches!(binding.mechanism, HostBindingMechanism::Syscall { .. }));
    // An AUTHORED import (custom capability + Import mechanism) always rides
    // the value-returning layout on aarch64 -- the operand fixups shift the
    // same way the BL does (see external_calls.rs).
    let authored_import = operation_key.is_some_and(|key| {
        matches!(
            key.capability,
            omega_calling_conventions::HostCapability::Custom(_)
                | omega_calling_conventions::HostCapability::Unknown
        ) && find_host_binding(input, key)
            .is_some_and(|binding| matches!(binding.mechanism, HostBindingMechanism::Import { .. }))
    });
    // A field-model call's fixup layout depends on the mechanism's shape:
    // whether the receiver is a wire argument (This-call vtable) or
    // dispatch-only (service table), and whether a result place leads the
    // operands (more operands than declared parameters).
    let field_model_shape = operation_key
        .and_then(|key| find_host_binding(input, key))
        .and_then(|binding| match &binding.mechanism {
            HostBindingMechanism::VtableSlot { .. } => Some(FieldModelCallShape {
                passes_receiver: true,
                result_present: false,
            }),
            HostBindingMechanism::VtableField {
                parameter_count, ..
            } => Some(FieldModelCallShape {
                passes_receiver: true,
                result_present: operands.len() > *parameter_count,
            }),
            HostBindingMechanism::TableFunction {
                parameter_count, ..
            } => Some(FieldModelCallShape {
                passes_receiver: false,
                result_present: operands.len() > *parameter_count,
            }),
            _ => None,
        });

    for (operand_index, operand) in operands.iter().enumerate() {
        if let Some(data) = operand.data_address() {
            if !data.is_valid() {
                continue;
            }
            let symbol = object_symbol_handle_by_name(
                &input.object,
                input.data.objects.get(data).symbol.as_ref(),
            );
            insert_data_address_relocations(
                input,
                relocation_plan,
                function_symbol_handle,
                selected_instruction_index,
                data_address_relocation_offset(
                    input.target.architecture,
                    operation_key,
                    operands,
                    selected_text_offset,
                    operand_index,
                    is_syscall,
                    field_model_shape,
                    authored_import,
                ),
                symbol,
            );
            continue;
        }

        let region = operand
            .runtime_string_pointer()
            .map(|(region, _)| region)
            .or_else(|| operand.runtime_string_length().map(|(region, _)| region))
            .or_else(|| {
                operand
                    .runtime_pointee_string_pointer()
                    .map(|(region, _)| region)
            })
            .or_else(|| {
                operand
                    .runtime_pointee_string_length()
                    .map(|(region, _)| region)
            })
            .or_else(|| {
                operand
                    .runtime_scalar_integer()
                    .map(|(region, _, _)| region)
            })
            .or_else(|| operand.runtime_scalar_float().map(|(region, _, _)| region))
            .or_else(|| {
                operand
                    .runtime_homogeneous_float_aggregate()
                    .map(|(region, _, _, _)| region)
            })
            .or_else(|| {
                operand
                    .runtime_small_aggregate()
                    .map(|(region, _, _, _)| region)
            })
            .or_else(|| operand.runtime_storage_address().map(|(region, _)| region));

        if let Some(region) = region {
            let symbol_name = storage_region_symbol_name(region, input.entry_machine_name);
            let symbol = object_symbol_handle_by_name(&input.object, &symbol_name);
            insert_data_address_relocations(
                input,
                relocation_plan,
                function_symbol_handle,
                selected_instruction_index,
                data_address_relocation_offset(
                    input.target.architecture,
                    operation_key,
                    operands,
                    selected_text_offset,
                    operand_index,
                    is_syscall,
                    field_model_shape,
                    authored_import,
                ),
                symbol,
            );
        }
    }
}
