use crate::RelocationPlanningInput;
use omega_calling_conventions::HostOperationKey;
use omega_instruction_selection as architecture;
use omega_object_file::{
    ObjectSymbolHandle, RelocationKind, RelocationPlan, RelocationRecord,
    object_symbol_handle_by_name, storage_region_symbol_name,
};
use omega_target::Architecture;
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
                    input,
                    operation_key,
                    operands,
                    selected_text_offset,
                    operand_index,
                ),
                symbol,
            );
            continue;
        }

        let region = operand
            .runtime_string_pointer()
            .map(|(region, _)| region)
            .or_else(|| operand.runtime_string_length().map(|(region, _)| region));

        if let Some(region) = region {
            let symbol_name = storage_region_symbol_name(region, input.entry_machine_name);
            let symbol = object_symbol_handle_by_name(&input.object, &symbol_name);
            insert_data_address_relocations(
                input,
                relocation_plan,
                function_symbol_handle,
                selected_instruction_index,
                data_address_relocation_offset(
                    input,
                    operation_key,
                    operands,
                    selected_text_offset,
                    operand_index,
                ),
                symbol,
            );
        }
    }
}

fn data_address_relocation_offset(
    input: RelocationPlanningInput<'_>,
    operation_key: Option<HostOperationKey>,
    operands: &[omega_assigned_target_operations::InstructionOperand],
    selected_text_offset: usize,
    operand_index: usize,
) -> usize {
    if input.target.architecture == Architecture::X86_64
        && let Some(operation_key) = operation_key
        && let Some(site) =
            omega_isa_x86_64::host_call_data_relocation_site(operation_key, operands, operand_index)
    {
        return selected_text_offset + site.byte_offset;
    }

    selected_text_offset
        + operands
            .iter()
            .take(operand_index)
            .map(|operand| architecture::operand_width(input.target.architecture, operand))
            .sum::<usize>()
}

pub(super) fn insert_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    relocation_plan: &mut RelocationPlan,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol_handle: ObjectSymbolHandle,
) {
    insert_data_address_relocations_for_architecture(
        input.target.architecture,
        relocation_plan,
        function_symbol_handle,
        selected_instruction_index,
        operand_text_offset,
        symbol_handle,
    );
}

fn insert_data_address_relocations_for_architecture(
    architecture: Architecture,
    relocation_plan: &mut RelocationPlan,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol_handle: ObjectSymbolHandle,
) {
    match architecture {
        Architecture::Aarch64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 4,
                symbol_handle,
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset + 4,
                byte_width: 4,
                symbol_handle,
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol_handle,
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 8,
                symbol_handle,
                kind: RelocationKind::X86_64Absolute64,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::insert_data_address_relocations_for_architecture;
    use omega_core::arena::{Arena, Handle};
    use omega_object_file::{RelocationKind, RelocationPlan};
    use omega_target::{Architecture, NativeTarget};

    #[test]
    fn inserts_target_specific_data_address_relocations() {
        let function_symbol = Handle::from_arena_index(1);
        let data_symbol = Handle::from_arena_index(2);

        let mut aarch64_plan = RelocationPlan {
            target: NativeTarget::linux_arm64(),
            records: Arena::new(),
        };
        insert_data_address_relocations_for_architecture(
            Architecture::Aarch64,
            &mut aarch64_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let aarch64_records: Vec<_> = aarch64_plan
            .records
            .iter()
            .map(|(_, record)| record)
            .collect();

        assert_eq!(aarch64_records.len(), 2);
        assert_eq!(aarch64_records[0].kind, RelocationKind::Aarch64Page21);
        assert_eq!(aarch64_records[0].text_offset, 12);
        assert_eq!(aarch64_records[0].byte_width, 4);
        assert_eq!(aarch64_records[1].kind, RelocationKind::Aarch64PageOffset12);
        assert_eq!(aarch64_records[1].text_offset, 16);
        assert_eq!(aarch64_records[1].byte_width, 4);

        let mut x86_plan = RelocationPlan {
            target: NativeTarget::windows_x64(),
            records: Arena::new(),
        };
        insert_data_address_relocations_for_architecture(
            Architecture::X86_64,
            &mut x86_plan,
            function_symbol,
            7,
            12,
            data_symbol,
        );
        let x86_records: Vec<_> = x86_plan.records.iter().map(|(_, record)| record).collect();

        assert_eq!(x86_records.len(), 1);
        assert_eq!(x86_records[0].kind, RelocationKind::X86_64Absolute64);
        assert_eq!(x86_records[0].text_offset, 12);
        assert_eq!(x86_records[0].byte_width, 8);
    }
}
