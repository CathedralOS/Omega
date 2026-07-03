use super::context::InstructionRelocationContext;
use super::runtime_values::collect_runtime_value_operand_relocations;
use crate::offsets::{
    runtime_frame_base_indexed_binary_left_operand_offset,
    runtime_frame_indexed_binary_left_operand_offset,
    runtime_machine_indexed_integer_runtime_frame_address_offset,
    runtime_pointee_binary_left_operand_offset, runtime_storage_binary_left_operand_offset,
};
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_write_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WriteRuntimeMachineInteger { .. } => {
            let symbol = context.machine_storage_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::WriteRuntimeStorageInteger { target_region, .. } => {
            let symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::WriteEntryArgumentRegister { .. }
        | SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. } => {
            // The entry prologue's `mov r15, imm64` materializes the RUNTIME
            // FRAME base (the entry parameters + the argument spill are frame
            // storage), anchored at the instruction start like every other
            // storage write.
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::WriteRuntimePointeeInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
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
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            let right_offset = left_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
            true
        }
        SelectedInstructionKind::WriteRuntimeStorageConvert {
            target_region,
            source,
            ..
        } => {
            // Same prefix as a storage binary write: `mov r14,imm64(target base)`
            // at the instruction start, then the (single) source operand.
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let source_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, source_offset, *source);
            true
        }
        SelectedInstructionKind::AtomicFetchAdd {
            target_region,
            delta,
            ..
        } => {
            // Same prefix as the converting/binary write: `mov r14,imm64(target
            // base)` at the instruction start, then the (single) `delta` operand
            // loaded at the same offset, then the `lock xadd`.
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let delta_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, delta_offset, *delta);
            true
        }
        SelectedInstructionKind::AtomicCompareExchange {
            target_region,
            expected,
            new_value,
            ..
        } => {
            // Target base relocated at the instruction start, then TWO operands:
            // `new_value` first (at the binary-write left-operand offset), then
            // `expected` immediately after it, then the `lock cmpxchg` / `casal`.
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let new_value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, new_value_offset, *new_value);
            // `expected` follows `new_value` plus the same stash gap the binary
            // write uses between its two operands (x86 `push r10`; 0 on aarch64,
            // where the operands occupy distinct registers).
            let expected_offset = new_value_offset
                + omega_instruction_selection::runtime_value_operand_width(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *new_value,
                )
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, expected_offset, *expected);
            true
        }
        SelectedInstructionKind::WriteRuntimePointeeBinary {
            pointer_byte_offset,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let left_offset = context.selected_text_offset
                + runtime_pointee_binary_left_operand_offset(
                    context.input.target.architecture,
                    *pointer_byte_offset,
                    *field_byte_offset,
                );
            collect_runtime_value_operand_relocations(context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            // The encoder stashes the left result (push r10) between the operands;
            // the right operand's bytes start after that gap.
            let right_offset = left_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameIndexedInteger { .. }
        | SelectedInstructionKind::WriteRuntimeFrameBaseIndexedInteger { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
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
                        context.input.target.architecture,
                        *base_byte_offset,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
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
                + runtime_frame_indexed_binary_left_operand_offset(
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                );
            collect_runtime_value_operand_relocations(context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            collect_runtime_value_operand_relocations(context, left_offset + left_width, *right);
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let left_offset = context.selected_text_offset
                + runtime_frame_base_indexed_binary_left_operand_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                    *index_offset,
                    *element_byte_size,
                    *field_byte_offset,
                );
            collect_runtime_value_operand_relocations(context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            let right_offset = left_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
            true
        }
        SelectedInstructionKind::WriteRuntimeMachineIndexedBinary {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            // Machine-region sibling of `WriteRuntimeFrameBaseIndexedBinary`: the
            // base at the instruction start (`mov r14, imm64`) relocates to the
            // MACHINE-storage symbol instead of the frame symbol. The byte layout
            // is identical, so the left/right value-operand offsets reuse the
            // frame-base helper. Only the Machine-resident index is emitted (the
            // frame-index case errors in the encoder), so there is no extra
            // frame-symbol relocation for the index.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            let left_offset = context.selected_text_offset
                + runtime_frame_base_indexed_binary_left_operand_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                    *index_offset,
                    *element_byte_size,
                    *field_byte_offset,
                );
            collect_runtime_value_operand_relocations(context, left_offset, *left);
            let left_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *left,
            );
            let right_offset = left_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
            true
        }
        _ => false,
    }
}
