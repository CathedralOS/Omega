use super::super::offsets::{
    runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
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
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                    context.input.target.architecture,
                    *base_byte_offset,
                ),
                context.runtime_frame_symbol_handle(),
            );
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
        SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee { source_region, .. } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_target_address_offset(context.input.target.architecture),
                context.runtime_frame_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame { .. } => {
            let symbol = context.runtime_frame_symbol_handle();
            context.insert_data_address_at_instruction_start(symbol);
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset(
                    context.input.target.architecture,
                ),
                symbol,
            );
            true
        }
        _ => false,
    }
}
