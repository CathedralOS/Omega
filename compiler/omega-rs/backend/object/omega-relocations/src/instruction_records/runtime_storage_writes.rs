use super::context::InstructionRelocationContext;
use super::runtime_values::collect_runtime_value_operand_relocations;
use crate::offsets::{
    runtime_frame_base_indexed_binary_left_operand_offset,
    runtime_frame_indexed_binary_left_operand_offset, runtime_pointee_binary_left_operand_offset,
    runtime_storage_binary_left_operand_offset,
};
use omega_target::Architecture;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_write_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WriteEntryArgumentRegister { .. }
        | SelectedInstructionKind::WriteEntryStackArgument { .. }
        | SelectedInstructionKind::WriteEntryArgumentsSliceDescriptor { .. } => {
            // The entry prologue's `mov r15, imm64` materializes the RUNTIME
            // FRAME base (the entry parameters + the argument spill are frame
            // storage), anchored at the instruction start like every other
            // storage write.
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::WriteEntryIndirectArgument { pointer, .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            let offset = omega_instruction_selection::entry_indirect_argument_frame_base_offset(
                context.input.target.architecture,
                *pointer,
            );
            context.insert_data_address_at_relative_offset(offset, symbol);
            true
        }
        SelectedInstructionKind::WriteStorageBitField { region, .. } => {
            context.insert_data_address_at_instruction_start(
                context.storage_region_symbol_handle(*region),
            );
            true
        }
        SelectedInstructionKind::WritePlaceBinary {
            target,
            left,
            right,
            ..
        } => {
            // Binary rung 2a. x86_64: the target base at instruction start +
            // each CROSS-REGION index's own base at its deterministic prefix
            // position, operands at place_binary_operand_start_width -- all
            // walk-summed from the materializer's own widths (no drift).
            // AArch64 is served by the same retained shape decomposition used
            // by encoding, including the extra MACHINE index base for a
            // frame-held descriptor indexed from machine storage.
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    for (position, region) in
                        omega_instruction_selection::place_binary_index_base_positions(target)
                    {
                        context.insert_data_address_at_relative_offset(
                            position,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    let left_offset = context.selected_text_offset
                        + omega_instruction_selection::place_binary_operand_start_width(target);
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
                }
                Architecture::Aarch64 => {
                    // Mirror the retained kinds' aarch64 arms by shape (the
                    // SAME classifier the encoder decomposes with).
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                    let shape = omega_instruction_selection::classify_write_place_shape(target);
                    let frame_indexed =
                        omega_instruction_selection::classify_frame_base_indexed_binary_shape(
                            target,
                        );
                    let frame_double =
                        omega_instruction_selection::classify_frame_base_double_indexed_binary_shape(
                            target,
                        );
                    let mut operand_start = match shape {
                        omega_instruction_selection::WritePlaceShape::Direct { .. } => {
                            runtime_storage_binary_left_operand_offset(
                                context.input.target.architecture,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::Pointee {
                            pointer_byte_offset,
                            field_byte_offset,
                        } => runtime_pointee_binary_left_operand_offset(
                            context.input.target.architecture,
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::FrameIndexed {
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => runtime_frame_indexed_binary_left_operand_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion {
                            index_region,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(index_region),
                            );
                            runtime_frame_indexed_binary_left_operand_offset(
                                context.input.target.architecture,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            ) + 8
                        }
                        omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => runtime_frame_base_indexed_binary_left_operand_offset(
                            context.input.target.architecture,
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::MachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => {
                            let mut start = runtime_frame_base_indexed_binary_left_operand_offset(
                                context.input.target.architecture,
                                base_byte_offset,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            );
                            if index_region == frame {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_machine_indexed_string_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                                start += 8;
                            }
                            start
                        }
                        omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed {
                            outer_index_region,
                            outer_index_byte_size,
                            inner_index_region,
                            inner_index_byte_size,
                            ..
                        } => {
                            if outer_index_region == frame || inner_index_region == frame {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            omega_instruction_selection::runtime_machine_double_indexed_binary_left_operand_offset(
                                context.input.target.architecture,
                                outer_index_region,
                                outer_index_byte_size,
                                inner_index_region,
                                inner_index_byte_size,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::PointeeDoubleIndexed {
                            ..
                        } => unreachable!(
                            "pointee-double-indexed binary writes refuse during aarch64 encoding"
                        ),
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_indexed.is_some() =>
                        {
                            let frame_indexed = frame_indexed
                                .expect("the guarded frame-base-indexed shape must be retained");
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::runtime_frame_base_indexed_machine_index_base_offset(
                                    context.input.target.architecture,
                                    frame_indexed.base_byte_offset,
                                ),
                                context.storage_region_symbol_handle(frame_indexed.index_region),
                            );
                            omega_instruction_selection::runtime_frame_base_indexed_operand_start_width_with_index_region(
                                context.input.target.architecture,
                                frame_indexed.base_byte_offset,
                                frame_indexed.index_region,
                                frame_indexed.index_offset,
                                frame_indexed.index_byte_size,
                                frame_indexed.element_byte_size,
                                frame_indexed.field_byte_offset,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_double.is_some() =>
                        {
                            omega_instruction_selection::runtime_frame_base_double_indexed_binary_left_operand_offset()
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported => {
                            unreachable!(
                                "an unsupported WritePlaceBinary shape refuses at \
                                 aarch64 encoding; layout would have failed first"
                            )
                        }
                    };
                    let left_offset = context.selected_text_offset + operand_start;
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
                    let _ = &mut operand_start;
                }
            }
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
        SelectedInstructionKind::WritePlaceConvert {
            target,
            target_byte_size,
            source,
            source_byte_size,
            source_is_float,
            target_is_float,
            source_signed,
            target_signed,
            trapping,
            saturating,
        } => {
            let operand_start = match context.input.target.architecture {
                Architecture::X86_64 => {
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_write_place_convert_with_sites(
                            context.input.assigned_target_operations,
                            target,
                            *target_byte_size,
                            *source,
                            *source_byte_size,
                            *source_is_float,
                            *target_is_float,
                            *source_signed,
                            *target_signed,
                            *trapping,
                            *saturating,
                        )
                        .expect("place convert reached relocation after successful layout");
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("TargetIndex implies a scaled target step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("TargetIndex2 implies two scaled target steps"),
                            _ => unreachable!("a place convert materializes only its target"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    omega_instruction_selection::place_binary_operand_start_width(target)
                }
                Architecture::Aarch64 => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    let shape = omega_instruction_selection::classify_write_place_shape(target);
                    let frame_indexed =
                        omega_instruction_selection::classify_frame_base_indexed_convert_shape(
                            target,
                        );
                    let frame_double =
                        omega_instruction_selection::classify_frame_base_double_indexed_convert_shape(
                            target,
                        );
                    match shape {
                        omega_instruction_selection::WritePlaceShape::Direct { .. } => 8,
                        omega_instruction_selection::WritePlaceShape::Pointee {
                            pointer_byte_offset,
                            field_byte_offset,
                        } => omega_instruction_selection::runtime_pointee_operand_start_width(
                            context.input.target.architecture,
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::FrameIndexed {
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => omega_instruction_selection::runtime_frame_indexed_binary_left_operand_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion {
                            index_region,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(index_region),
                            );
                            omega_instruction_selection::runtime_frame_indexed_binary_left_operand_offset(
                                context.input.target.architecture,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                            ) + 8
                        }
                        omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => omega_instruction_selection::runtime_frame_base_indexed_binary_left_operand_offset(
                            context.input.target.architecture,
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        omega_instruction_selection::WritePlaceShape::MachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => {
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_machine_indexed_integer_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            omega_instruction_selection::runtime_machine_indexed_integer_write_width(
                                context.input.target.architecture,
                                base_byte_offset,
                                index_region,
                                index_offset,
                                index_byte_size,
                                element_byte_size,
                                field_byte_offset,
                                0,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed {
                            outer_index_region,
                            outer_index_byte_size,
                            inner_index_region,
                            inner_index_byte_size,
                            ..
                        } => {
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            omega_instruction_selection::runtime_machine_double_indexed_binary_left_operand_offset(
                                context.input.target.architecture,
                                outer_index_region,
                                outer_index_byte_size,
                                inner_index_region,
                                inner_index_byte_size,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::PointeeDoubleIndexed {
                            ..
                        } => unreachable!(
                            "pointee-double-indexed convert writes refuse during aarch64 encoding"
                        ),
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_indexed.is_some() =>
                        {
                            let frame_indexed = frame_indexed
                                .expect("guarded frame-base-indexed conversion target");
                            if frame_indexed.index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_frame_base_indexed_machine_index_base_offset(
                                        context.input.target.architecture,
                                        frame_indexed.base_byte_offset,
                                    ),
                                    context.machine_storage_symbol_handle(),
                                );
                            }
                            omega_instruction_selection::runtime_frame_base_indexed_operand_start_width_with_index_region(
                                context.input.target.architecture,
                                frame_indexed.base_byte_offset,
                                frame_indexed.index_region,
                                frame_indexed.index_offset,
                                frame_indexed.index_byte_size,
                                frame_indexed.element_byte_size,
                                frame_indexed.field_byte_offset,
                            )
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_double.is_some() =>
                        {
                            omega_instruction_selection::runtime_frame_base_double_indexed_convert_operand_offset()
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported => {
                            unreachable!("unsupported aarch64 place convert refuses during layout")
                        }
                    }
                }
            };
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset + operand_start,
                *source,
            );
            true
        }
        SelectedInstructionKind::AtomicLoad {
            source_region,
            source_offset,
            byte_size,
            result_region,
            ..
        } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_load_result_address_offset(
                    context.input.target.architecture,
                    *source_offset,
                    *byte_size,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicStore {
            target_region,
            value,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, value_offset, *value);
            true
        }
        SelectedInstructionKind::AtomicFetchAdd {
            target_region,
            target_offset,
            byte_size,
            result_region,
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
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_fetch_add_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *delta,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicFetchSub {
            target_region,
            target_offset,
            byte_size,
            result_region,
            delta,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let delta_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, delta_offset, *delta);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_fetch_sub_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *delta,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicFetchXor {
            target_region,
            target_offset,
            byte_size,
            result_region,
            value,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, value_offset, *value);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_fetch_xor_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *value,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicFetchOr {
            target_region,
            target_offset,
            byte_size,
            result_region,
            value,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, value_offset, *value);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_fetch_or_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *value,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicFetchAnd {
            target_region,
            target_offset,
            byte_size,
            result_region,
            value,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, value_offset, *value);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_fetch_and_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *value,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicSwap {
            target_region,
            target_offset,
            byte_size,
            result_region,
            new_value,
            ..
        } => {
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(target_symbol);
            let value_offset = context.selected_text_offset
                + runtime_storage_binary_left_operand_offset(context.input.target.architecture);
            collect_runtime_value_operand_relocations(context, value_offset, *new_value);
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_swap_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *new_value,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::AtomicCompareExchange {
            target_region,
            target_offset,
            byte_size,
            result_region,
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
            let result_symbol = context.storage_region_symbol_handle(*result_region);
            let result_address_offset =
                omega_instruction_selection::runtime_atomic_compare_exchange_result_address_offset(
                    context.input.target.architecture,
                    context.input.assigned_target_operations,
                    *target_offset,
                    *byte_size,
                    *expected,
                    *new_value,
                );
            context.insert_data_address_at_relative_offset(result_address_offset, result_symbol);
            true
        }
        SelectedInstructionKind::PortWrite { port, value } => {
            // `out dx, al`: the port operand load starts at the instruction
            // start; the value operand load follows it plus the DX register
            // move. Storage operands relocate; immediates are no-ops. There is
            // no target storage region (the write goes to a hardware port).
            let port_offset = context.selected_text_offset;
            collect_runtime_value_operand_relocations(context, port_offset, *port);
            let port_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *port,
            );
            let value_offset =
                port_offset + port_width + omega_isa_x86_64::PORT_OPERAND_REGISTER_MOVE_WIDTH;
            collect_runtime_value_operand_relocations(context, value_offset, *value);
            true
        }
        SelectedInstructionKind::PortRead {
            port, dest_region, ..
        } => {
            // `in al, dx`: the port operand load, then `in al, dx` (1 byte),
            // then the destination store whose `mov r15,imm64` relocates to the
            // destination storage region.
            let port_offset = context.selected_text_offset;
            collect_runtime_value_operand_relocations(context, port_offset, *port);
            let port_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *port,
            );
            let dest_store_relative =
                port_width + omega_isa_x86_64::PORT_OPERAND_REGISTER_MOVE_WIDTH + 1;
            let dest_symbol = context.storage_region_symbol_handle(*dest_region);
            context.insert_data_address_at_relative_offset(dest_store_relative, dest_symbol);
            true
        }
        SelectedInstructionKind::FlagsSnapshot { dest_region, .. } => {
            let dest_symbol = context.storage_region_symbol_handle(*dest_region);
            context.insert_data_address_at_relative_offset(
                omega_isa_x86_64::FLAGS_SNAPSHOT_DESTINATION_BASE_OFFSET,
                dest_symbol,
            );
            true
        }
        SelectedInstructionKind::FlagsRestore { source } => {
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset,
                *source,
            );
            true
        }
        SelectedInstructionKind::MsrRead {
            index, dest_region, ..
        } => {
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset,
                *index,
            );
            let dest_symbol = context.storage_region_symbol_handle(*dest_region);
            let dest_relative = omega_isa_x86_64::msr_read_destination_base_offset(
                context.input.assigned_target_operations,
                *index,
            );
            context.insert_data_address_at_relative_offset(dest_relative, dest_symbol);
            true
        }
        SelectedInstructionKind::MsrWrite { index, value } => {
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset,
                *index,
            );
            let index_width = omega_instruction_selection::runtime_value_operand_width(
                context.input.target.architecture,
                context.input.assigned_target_operations,
                *index,
            );
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset
                    + index_width
                    + omega_isa_x86_64::MSR_WRITE_INDEX_STASH_WIDTH,
                *value,
            );
            true
        }
        SelectedInstructionKind::ControlRegisterRead { dest_region, .. } => {
            let dest_symbol = context.storage_region_symbol_handle(*dest_region);
            context.insert_data_address_at_relative_offset(
                omega_isa_x86_64::CONTROL_REGISTER_READ_DESTINATION_BASE_OFFSET,
                dest_symbol,
            );
            true
        }
        SelectedInstructionKind::ControlRegisterWrite { source, .. } => {
            collect_runtime_value_operand_relocations(
                context,
                context.selected_text_offset,
                *source,
            );
            true
        }
        _ => false,
    }
}
