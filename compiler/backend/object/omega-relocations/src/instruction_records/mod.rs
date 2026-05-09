mod context;
mod host_operation;
mod runtime_text;

use super::data_addresses::collect_data_address_relocations;
use super::offsets::{
    runtime_storage_compare_right_address_offset, runtime_storage_copy_target_address_offset,
    string_descriptor_machine_address_offset,
};
use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_object::{RelocationPlan, machine_storage_symbol_name, storage_region_symbol_name};
use omega_target_program::{
    FunctionInstructionPlan, SelectedInstruction, SelectedInstructionKind, StateGuardLowering,
    StateGuardOperator,
};

pub(super) fn collect_instruction_relocations(
    input: RelocationPlanningInput<'_>,
    function: &FunctionInstructionPlan,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) {
    let mut context = InstructionRelocationContext {
        input,
        function,
        selected_instruction_index,
        selected_text_offset,
        relocation_plan,
    };

    match &instruction.kind {
        SelectedInstructionKind::HostOperation { operands, .. } => {
            collect_data_address_relocations(
                input,
                function,
                selected_instruction_index,
                *operands,
                selected_text_offset,
                context.relocation_plan,
            );
            host_operation::collect_host_operation_relocation(&mut context, &instruction.kind);
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: StateGuardOperator::Equal | StateGuardOperator::NotEqual,
            has_storage: true,
            ..
        } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                input.entry_machine_name,
            ));
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_region,
            right_region,
            ..
        } => {
            let left_symbol = storage_region_symbol_name(*left_region, input.entry_machine_name);
            let right_symbol = storage_region_symbol_name(*right_region, input.entry_machine_name);
            context.insert_data_address_at_instruction_start(&left_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_compare_right_address_offset(input.target.architecture),
                &right_symbol,
            );
        }
        SelectedInstructionKind::CompareRuntimeStorageValue { region, .. } => {
            let symbol = storage_region_symbol_name(*region, input.entry_machine_name);
            context.insert_data_address_at_instruction_start(&symbol);
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
            context.insert_data_address_at_instruction_start(&machine_storage_symbol_name(
                input.entry_machine_name,
            ));
        }
        SelectedInstructionKind::WriteRuntimeMachineString { data, .. } => {
            let data_symbol = &input.data.objects.get(*data).symbol;
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_machine_address_offset(input.target.architecture),
                &machine_storage_symbol_name(input.entry_machine_name),
            );
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_region,
            target_region,
            ..
        } => {
            let source_symbol =
                storage_region_symbol_name(*source_region, input.entry_machine_name);
            let target_symbol =
                storage_region_symbol_name(*target_region, input.entry_machine_name);
            context.insert_data_address_at_instruction_start(&source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(input.target.architecture),
                &target_symbol,
            );
        }
        _ => runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind),
    }
}
