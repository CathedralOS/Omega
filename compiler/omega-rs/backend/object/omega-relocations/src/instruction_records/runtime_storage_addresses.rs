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
        SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            index_region, ..
        } => {
            // Source frame base at the start; x86_64 reloads the frame base for
            // the target store, and a MACHINE-resident index loads the machine
            // base at +10 (its own relocation). On aarch64 a machine-resident
            // index materializes its page pair at the constant +32 (after the
            // frame pair + the fixed-width descriptor load).
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            if context.input.target.architecture == omega_target::Architecture::Aarch64
                && *index_region == omega_target_operations::RuntimeStorageRegion::Machine
            {
                context.insert_data_address_at_relative_offset(
                    32,
                    context.machine_storage_symbol_handle(),
                );
            }
            if context.input.target.architecture == omega_target::Architecture::X86_64 {
                if *index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    context.insert_data_address_at_relative_offset(
                        10,
                        context.machine_storage_symbol_handle(),
                    );
                }
                if let Some(offset) =
                    omega_instruction_selection::runtime_frame_indexed_deref_address_target_frame_offset(
                        context.input.target.architecture,
                        *index_region,
                    )
                {
                    context.insert_data_address_at_relative_offset(
                        offset,
                        context.runtime_frame_symbol_handle(),
                    );
                }
            }
            true
        }
        SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame { .. }
        | SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame { .. } => {
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            true
        }
        _ => false,
    }
}
