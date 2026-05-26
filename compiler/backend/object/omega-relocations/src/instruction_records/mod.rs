mod context;
mod host_operation;
mod runtime_text;

use super::data_addresses::collect_data_address_relocations;
use super::offsets::{
    runtime_frame_indexed_string_data_address_offset,
    runtime_machine_indexed_string_data_address_offset,
    runtime_machine_indexed_string_runtime_frame_address_offset,
    runtime_storage_compare_right_address_offset,
    runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
    runtime_storage_copy_target_address_offset, string_descriptor_machine_address_offset,
    string_descriptor_pointee_address_offset,
};
use crate::RelocationPlanningInput;
use context::InstructionRelocationContext;
use omega_assigned_target_operations::RuntimeValueOperand;
use omega_instruction_selection::runtime_machine_indexed_integer_runtime_frame_address_offset;
use omega_object_file::{ObjectSymbolHandle, RelocationPlan};
use omega_target::Architecture;
use omega_target_operations::{
    RuntimeValueOperandHandle, SelectedInstruction, SelectedInstructionKind, StateGuardLowering,
    StateGuardOperator,
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
        SelectedInstructionKind::HostOperation {
            operation_key,
            operands,
        } => {
            collect_data_address_relocations(
                input,
                function_symbol_handle,
                selected_instruction_index,
                Some(*operation_key),
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
        SelectedInstructionKind::CompareRuntimeValues { left, right, .. } => {
            let base_offset = context.selected_text_offset;
            collect_runtime_value_operand_relocations(&mut context, base_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                input.target.architecture,
                input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                &mut context,
                base_offset + left_width,
                *right,
            );
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
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_region,
            left,
            right,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let left_offset = context.selected_text_offset
                + match input.target.architecture {
                    Architecture::Aarch64 => 8,
                    Architecture::X86_64 => 10,
                };
            collect_runtime_value_operand_relocations(&mut context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                input.target.architecture,
                input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                &mut context,
                left_offset + left_width,
                *right,
            );
        }
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            field_byte_offset,
            left,
            right,
            ..
        } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let left_offset = context.selected_text_offset
                + omega_instruction_selection::runtime_pointee_operand_start_width(
                    input.target.architecture,
                    *field_byte_offset,
                );
            collect_runtime_value_operand_relocations(&mut context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                input.target.architecture,
                input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                &mut context,
                left_offset + left_width,
                *right,
            );
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::WriteRuntimeMachineIndexedInteger {
            base_byte_offset,
            index_region,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if *index_region == omega_assigned_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                context.insert_data_address_at_relative_offset(
                    runtime_machine_indexed_integer_runtime_frame_address_offset(
                        input.target.architecture,
                        *base_byte_offset,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedBinary {
            element_byte_size,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let left_offset = context.selected_text_offset
                + omega_instruction_selection::runtime_frame_indexed_integer_write_width(
                    input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                    0,
                );
            collect_runtime_value_operand_relocations(&mut context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                input.target.architecture,
                input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                &mut context,
                left_offset + left_width,
                *right,
            );
        }
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let left_offset = context.selected_text_offset
                + omega_instruction_selection::runtime_frame_base_indexed_integer_write_width(
                    input.target.architecture,
                    *base_byte_offset,
                    *element_byte_size,
                    *field_byte_offset,
                    0,
                );
            collect_runtime_value_operand_relocations(&mut context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                input.target.architecture,
                input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                &mut context,
                left_offset + left_width,
                *right,
            );
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
        SelectedInstructionKind::WriteRuntimeFrameIndexedString {
            data,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_relative_offset(
                runtime_frame_indexed_string_data_address_offset(
                    input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                data_symbol,
            );
        }
        SelectedInstructionKind::WriteRuntimeMachineIndexedString {
            data,
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_machine_indexed_string_runtime_frame_address_offset(
                    input.target.architecture,
                    *base_byte_offset,
                ),
                context.runtime_frame_symbol_handle(),
            );
            let data_symbol = context.data_object_symbol_handle(*data);
            context.insert_data_address_at_relative_offset(
                runtime_machine_indexed_string_data_address_offset(
                    input.target.architecture,
                    *base_byte_offset,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                data_symbol,
            );
        }
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region, ..
        } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            let frame_offset = match input.target.architecture {
                Architecture::Aarch64 => 12,
                Architecture::X86_64 => 17,
            };
            context.insert_data_address_at_relative_offset(
                frame_offset,
                context.runtime_frame_symbol_handle(),
            );
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame { .. } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame { .. } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
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
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
            target_region,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                    input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
        }
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
        }
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            target_region,
            element_index,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset(
                    input.target.architecture,
                    *element_index,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
        }
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            target_region,
            base_byte_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                    input.target.architecture,
                    *base_byte_offset,
                ),
                context.runtime_frame_symbol_handle(),
            );
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                    input.target.architecture,
                    *base_byte_offset,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
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

fn collect_runtime_value_operand_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    operand_text_offset: usize,
    operand: RuntimeValueOperandHandle,
) {
    let Some(operand) = context
        .input
        .assigned_target_operations
        .runtime_value_operand(operand)
    else {
        return;
    };

    match &operand.kind {
        RuntimeValueOperand::Immediate(_) => {}
        RuntimeValueOperand::Storage { region, .. } => {
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address(operand_text_offset, symbol);
        }
        RuntimeValueOperand::Pointee { .. }
        | RuntimeValueOperand::FrameBaseIndexed { .. }
        | RuntimeValueOperand::FrameIndexed { .. }
        | RuntimeValueOperand::FrameFixedIndexed { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address(operand_text_offset, symbol);
        }
        RuntimeValueOperand::Binary { left, right, .. } => {
            collect_runtime_value_operand_relocations(context, operand_text_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(
                context,
                operand_text_offset + left_width,
                *right,
            );
        }
    }
}
