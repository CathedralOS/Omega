use super::context::InstructionRelocationContext;
use super::runtime_values::collect_runtime_value_operand_relocations;
use crate::offsets::{
    runtime_frame_base_indexed_binary_left_operand_offset,
    runtime_frame_indexed_binary_left_operand_offset,
    runtime_machine_indexed_integer_runtime_frame_address_offset,
    runtime_pointee_binary_left_operand_offset, runtime_storage_binary_left_operand_offset,
};
use omega_target::Architecture;
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
        SelectedInstructionKind::WriteRuntimeMachineDoubleIndexedBinary {
            outer_index_region,
            inner_index_region,
            left,
            right,
            ..
        } => {
            // Machine base at the instruction start; ONE shared frame base at
            // +10 when either index is frame-resident (the double prologue's
            // r10 load); then the left/right value operands at the double
            // prologue's end.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if [outer_index_region, inner_index_region].iter().any(|region| {
                **region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            }) {
                context.insert_data_address_at_relative_offset(
                    // The shared frame base: x86_64 the `mov r10,imm64` at +10;
                    // aarch64 the page pair directly after the machine pair.
                    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            let left_offset = context.selected_text_offset
                + omega_instruction_selection::runtime_machine_double_indexed_binary_left_operand_offset(
                    context.input.target.architecture,
                    *outer_index_region,
                    *inner_index_region,
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
            index_region,
            element_byte_size,
            field_byte_offset,
            left,
            right,
            ..
        } => {
            // Machine-region sibling of `WriteRuntimeFrameBaseIndexedBinary`: the
            // base at the instruction start (`mov r14, imm64`) relocates to the
            // MACHINE-storage symbol instead of the frame symbol. The byte layout
            // matches, so the left/right value-operand offsets reuse the
            // frame-base helper. A FRAME-resident index (x86_64) inserts a
            // `mov r15,imm64` frame-base load at +10, shifting the operands.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            // A FRAME-resident index adds a frame-base materialization: x86_64
            // a `mov r15,imm64` at +10; aarch64 the page pair at the same
            // constant the machine-indexed string write uses (after the
            // machine pair + mov + base add), shifting the operands by 8.
            let frame_index_shift = if *index_region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                let (frame_offset, shift) = match context.input.target.architecture {
                    Architecture::X86_64 => (10, 10),
                    Architecture::Aarch64 => (
                        omega_instruction_selection::runtime_machine_indexed_string_runtime_frame_address_offset(
                            context.input.target.architecture,
                            *base_byte_offset,
                        ),
                        8,
                    ),
                };
                context.insert_data_address_at_relative_offset(
                    frame_offset,
                    context.runtime_frame_symbol_handle(),
                );
                shift
            } else {
                0
            };
            let left_offset = context.selected_text_offset
                + frame_index_shift
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
