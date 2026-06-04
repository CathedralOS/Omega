use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_address_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region,
            source_offset,
            ..
        } => {
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            context.insert_data_address_at_instruction_start(source_symbol);
            let frame_offset =
                omega_instruction_selection::runtime_storage_address_to_runtime_frame_target_frame_offset(
                    context.input.target.architecture,
                    *source_offset,
                );
            context.insert_data_address_at_relative_offset(
                frame_offset,
                context.runtime_frame_symbol_handle(),
            );
            true
        }
        SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame { .. } => {
            // Source frame base (the element-address computation) at the start.
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            // Architectures that reload the frame base for the target slot store
            // (x86_64) need a second relocation at that load's immediate.
            if let Some(offset) =
                omega_instruction_selection::runtime_frame_base_indexed_address_target_frame_offset(
                    context.input.target.architecture,
                )
            {
                context.insert_data_address_at_relative_offset(
                    offset,
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame { .. }
        | SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame { .. }
        | SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame { .. } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            true
        }
        _ => false,
    }
}
