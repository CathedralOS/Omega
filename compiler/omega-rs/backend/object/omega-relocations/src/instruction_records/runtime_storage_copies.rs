use super::super::offsets::{
    runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
    runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset,
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset,
    runtime_storage_copy_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_copy_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::CopyRuntimeStorage {
            source_region,
            target_region,
            ..
        } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(context.input.target.architecture),
                target_symbol,
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeStorageToReturnRegister { region, .. } => {
            // The terminal-value load's leading region-base materialization
            // (adrp+add on aarch64, `mov r15, imm64` on x86_64) anchors at the
            // instruction start, like every other storage read.
            let symbol = context.storage_region_symbol_handle(*region);
            context.insert_data_address_at_instruction_start(symbol);
            true
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
        | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
        | SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
        | SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimePointee { .. }
        | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimePointee { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            true
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
                    context.input.target.architecture,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
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
                    context.input.target.architecture,
                    *element_index,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            target_region,
            base_byte_offset,
            index_region,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            // The runtime-frame base is only loaded (and thus only relocated)
            // when the index itself is frame-resident; a machine-resident index
            // reads from the machine base, so a program with no frame storage at
            // all still relocates cleanly.
            if *index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                        context.input.target.architecture,
                        *base_byte_offset,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                    *element_byte_size,
                    *field_byte_offset,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeMachineIndexed { .. } => {
            // Single shared machine base: the only relocation is the machine
            // symbol at the `mov r15,imm64` that opens the instruction (the
            // planner offsets the imm itself). The source value, the index, and
            // the target element all read off this one base.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            true
        }
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeMachineIndexed { .. } => {
            // TWO machine-base relocations: the read part's `mov r15,imm64` at
            // instruction start and the write part's at +34 (both the machine
            // symbol -- source and target elements share the machine region).
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                    context.input.target.architecture,
                ),
                context.machine_storage_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee { source_region, .. } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(context.input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame { target_region, .. } => {
            // Source pointer lives in the FRAME; the copied bytes land in
            // `target_region` (frame or machine statics).
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset(
                    context.input.target.architecture,
                ),
                target_symbol,
            );
            true
        }
        _ => false,
    }
}
