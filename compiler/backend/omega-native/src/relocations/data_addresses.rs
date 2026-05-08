use crate::architecture;
use crate::instructions::{FunctionInstructionPlan, InstructionOperandKind};
use crate::object::machine_storage_symbol_name;
use crate::plan::NativePlan;
use omega_object::{RelocationKind, RelocationPlan, RelocationRecord};
use omega_target::Architecture;

pub(super) fn collect_data_address_relocations(
    native_plan: &NativePlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operands: omega_core::arena::HandleSpan<crate::instructions::InstructionOperand>,
    selected_text_offset: usize,
    relocation_plan: &mut RelocationPlan,
) {
    let Some(operands) = native_plan.instructions.operands.span(operands) else {
        return;
    };

    let mut operand_text_offset = selected_text_offset;

    for operand in operands {
        match &operand.kind {
            InstructionOperandKind::DataAddress { symbol } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    symbol,
                );
            }
            InstructionOperandKind::RuntimeMachineStringPointer { .. }
            | InstructionOperandKind::RuntimeMachineStringLength { .. } => {
                insert_data_address_relocations(
                    native_plan.target.architecture,
                    relocation_plan,
                    function,
                    selected_instruction_index,
                    operand_text_offset,
                    &machine_storage_symbol_name(&native_plan.entry_machine),
                );
            }
            InstructionOperandKind::ImmediateInteger(_) | InstructionOperandKind::ByteLength(_) => {
            }
        }

        operand_text_offset +=
            architecture::operand_width(native_plan.target.architecture, operand);
    }
}

pub(super) fn insert_data_address_relocations(
    architecture: Architecture,
    relocation_plan: &mut RelocationPlan,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    operand_text_offset: usize,
    symbol: &str,
) {
    match architecture {
        Architecture::Aarch64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 4,
                symbol: symbol.to_owned(),
                kind: RelocationKind::Aarch64Page21,
            });
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset + 4,
                byte_width: 4,
                symbol: symbol.to_owned(),
                kind: RelocationKind::Aarch64PageOffset12,
            });
        }
        Architecture::X86_64 => {
            relocation_plan.records.insert(RelocationRecord {
                function_symbol: function.symbol.clone(),
                selected_instruction_index,
                text_offset: operand_text_offset,
                byte_width: 8,
                symbol: symbol.to_owned(),
                kind: RelocationKind::X86_64Absolute64,
            });
        }
    }
}
