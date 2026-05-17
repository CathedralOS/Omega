use crate::RelocationPlanningInput;
use omega_instruction_selection as architecture;
use omega_object::{
    RelocationKind, RelocationPlan, RelocationRecord, object_symbol_handle_by_name,
    storage_region_symbol_name,
};
use omega_target::Architecture;
use omega_target_operations::{FunctionInstructionPlan, InstructionOperandKind};

pub(super) fn collect_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operands: omega_core::arena::HandleSpan<omega_target_operations::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = input.instructions.operands.span(operands) else {
        return;
    };

    let mut operand_text_offset = selected_text_offset;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::DataAddress { data } => {
                if !data.is_valid() {
                    continue;
                }
                let symbol = &input.data.objects.get(*data).symbol;
                insert_data_address_relocations(
                    input,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    symbol,
                );
            }
            InstructionOperandKind::RuntimeStringPointer { region, .. }
            | InstructionOperandKind::RuntimeStringLength { region, .. } => {
                insert_data_address_relocations(
                    input,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    &storage_region_symbol_name(*region, input.entry_machine_name),
                );
            }
            InstructionOperandKind::ImmediateInteger(_) | InstructionOperandKind::ByteLength(_) => {
            }
        }

        operand_text_offset += architecture::operand_width(input.target.architecture, operand);
    }
}

pub(super) fn insert_data_address_relocations(
    input: RelocationPlanningInput<'_>,
    relocation_plan: &mut RelocationPlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol: &str,
) {
    let symbol_handle = object_symbol_handle_by_name(&input.object, symbol);

    match input.target.architecture {
        Architecture::Aarch64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.to_string(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 4,
                symbol: symbol.to_owned(),
                symbol_handle,
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.to_string(),
                selected_instruction_index,
                text_offset: operand_text_offset + 4,
                byte_width: 4,
                symbol: symbol.to_owned(),
                symbol_handle,
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.to_string(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 8,
                symbol: symbol.to_owned(),
                symbol_handle,
                kind: RelocationKind::X86_64Absolute64,
            });
        }
    }
}
