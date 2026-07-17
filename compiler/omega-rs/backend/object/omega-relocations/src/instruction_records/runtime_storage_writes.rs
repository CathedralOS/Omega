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
            // aarch64 is served by shape decompose at ENCODING; its reloc
            // arm lands with the producers (zero exist yet) -- refuse loudly
            // rather than silently under-patch.
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
                    unreachable!(
                        "WritePlaceBinary has no aarch64 producers yet; its reloc arm \
                         lands with the producer migration"
                    )
                }
            }
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
            // x86_64 pushes the left result between the operands (2 bytes);
            // aarch64's gap is 0, so this is identity there.
            let right_offset = left_offset
                + left_width
                + omega_instruction_selection::runtime_binary_right_operand_gap(
                    context.input.target.architecture,
                );
            collect_runtime_value_operand_relocations(context, right_offset, *right);
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
            let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
            match context.input.target.architecture {
                // aarch64 keeps its retired shared-base layout.
                Architecture::Aarch64 => {
                    if *outer_index_region == frame || *inner_index_region == frame {
                        context.insert_data_address_at_relative_offset(
                            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                context.input.target.architecture,
                            ),
                            context.runtime_frame_symbol_handle(),
                        );
                    }
                }
                // x86_64 canonicalized (Binary rung 1b): each frame-resident
                // index materializes its OWN base (r11 outer, r10 inner) at
                // the SAME positions the integer double write uses.
                Architecture::X86_64 => {
                    if *outer_index_region == frame {
                        context.insert_data_address_at_relative_offset(
                            omega_instruction_selection::runtime_machine_double_indexed_integer_write_outer_frame_offset(),
                            context.runtime_frame_symbol_handle(),
                        );
                    }
                    if *inner_index_region == frame {
                        context.insert_data_address_at_relative_offset(
                            omega_instruction_selection::runtime_machine_double_indexed_integer_write_inner_frame_offset(
                                *outer_index_region,
                            ),
                            context.runtime_frame_symbol_handle(),
                        );
                    }
                }
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
        _ => false,
    }
}
