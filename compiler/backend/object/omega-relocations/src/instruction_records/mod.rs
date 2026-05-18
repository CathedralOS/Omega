mod context;
mod host_operation;
mod runtime_text;

use super::data_addresses::collect_data_address_relocations;
use super::offsets::{
    runtime_storage_compare_right_address_offset, runtime_storage_copy_target_address_offset,
    string_descriptor_machine_address_offset, string_descriptor_pointee_address_offset,
};
use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_object::{ObjectSymbolHandle, RelocationPlan};
use omega_target_operations::{
    SelectedInstruction, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};

pub(super) fn collect_instruction_relocations(
    input: RelocationPlanningInput<'_>,
    function_symbol_handle: ObjectSymbolHandle,
    selected_instruction_index: u32,
    selected_text_offset: usize,
    instruction: &SelectedInstruction,
    relocation_plan: &mut RelocationPlan,
) {
    let mut context = InstructionRelocationContext {
        input,
        function_symbol_handle,
        selected_instruction_index,
        selected_text_offset,
        relocation_plan,
    };

    match &instruction.kind {
        SelectedInstructionKind::HostOperation { operands, .. } => {
            collect_data_address_relocations(
                input,
                function_symbol_handle,
                selected_instruction_index,
                *operands,
                selected_text_offset,
                context.relocation_plan,
            );
            host_operation::collect_host_operation_relocation(&mut context, &instruction.kind);
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual,
            storage_region,
            has_storage: true,
            ..
        } => {
            let symbol = context.storage_region_symbol_handle(*storage_region);
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_region,
            right_region,
            ..
        } => {
            let left_symbol = context.storage_region_symbol_handle(*left_region);
            let right_symbol = context.storage_region_symbol_handle(*right_region);
            context.insert_data_address_at_instruction_start(left_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_compare_right_address_offset(input.target.architecture),
                right_symbol,
            );
        }
        SelectedInstructionKind::CompareRuntimeStorageValue { region, .. } => {
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
            let symbol = context.machine_storage_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeStorageInteger { target_region, .. } => {
            let symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimePointeeInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimePointeeBinary { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeMachineString { data, .. } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_machine_address_offset(input.target.architecture),
                context.machine_storage_symbol_handle(),
            );
        }
        SelectedInstructionKind::WriteRuntimePointeeString { data, .. } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_instruction_start(data_symbol);
            context.insert_data_address_at_relative_offset(
                string_descriptor_pointee_address_offset(input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
        }
        SelectedInstructionKind::CopyRuntimeStorage {
            source_region,
            target_region,
            ..
        } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(input.target.architecture),
                target_symbol,
            );
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee { source_region, .. } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
        }
        _ => runtime_text::collect_runtime_text_relocations(&mut context, &instruction.kind),
    }
}
