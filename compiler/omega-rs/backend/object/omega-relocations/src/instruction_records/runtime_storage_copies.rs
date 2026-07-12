use super::super::offsets::{
    runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset,
    runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset,
    runtime_storage_copy_machine_indexed_frame_index_offset,
    runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset,
    runtime_storage_copy_to_runtime_machine_indexed_frame_index_base_offset,
    runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset,
    runtime_storage_copy_to_runtime_machine_indexed_source_address_offset,
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset,
    runtime_storage_copy_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target::Architecture;
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
            index_offset,
            element_byte_size,
            field_byte_offset,
            byte_count,
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
                    *index_region,
                    *index_offset,
                    *element_byte_size,
                    *field_byte_offset,
                    *byte_count,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeFrameBaseIndexedToRuntimeFrame { .. } => {
            // Source frame base (the element-address computation) at the start.
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            // Architectures that reload the frame base for the target slot store
            // (x86_64) need a second relocation at that load's immediate.
            if let Some(offset) =
                omega_instruction_selection::runtime_storage_copy_from_runtime_frame_base_indexed_target_frame_offset(
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
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeMachineIndexed {
            source_region,
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            if context.input.target.architecture == Architecture::Aarch64 {
                // aarch64 store layout: `[index-address setup (adrp x16 = machine
                // index base)] [source adrp x20] [load src / store elem]`. So the
                // machine page-pair is relocated at instruction start (the index
                // base) and again at the source adrp; a frame-resident INDEX adds
                // its own frame page-pair between them (same offset as the read).
                context
                    .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
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
                    runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                        context.input.target.architecture,
                        *base_byte_offset,
                        *index_region,
                        *index_offset,
                        *element_byte_size,
                        *field_byte_offset,
                    ),
                    context.storage_region_symbol_handle(*source_region),
                );
            } else {
                // x86_64: the opening `mov r15,imm64` loads the SOURCE base -- the
                // machine symbol for a field source (index + target element ride
                // it), or the runtime frame for a slot-backed local source, in
                // which case a SECOND relocation re-loads the machine base. A
                // FRAME-resident index adds its own frame-base `mov r10,imm64`.
                context.insert_data_address_at_instruction_start(
                    context.storage_region_symbol_handle(*source_region),
                );
                if *source_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    context.insert_data_address_at_relative_offset(
                        runtime_storage_copy_to_runtime_machine_indexed_frame_source_machine_base_offset(
                            context.input.target.architecture,
                        ),
                        context.machine_storage_symbol_handle(),
                    );
                }
                if *index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    context.insert_data_address_at_relative_offset(
                        runtime_storage_copy_to_runtime_machine_indexed_frame_index_base_offset(
                            context.input.target.architecture,
                            *source_region,
                        ),
                        context.runtime_frame_symbol_handle(),
                    );
                }
            }
            true
        }
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeMachineIndexed {
            source_index_region,
            target_index_region,
            ..
        } => {
            // TWO machine-base relocations: the read part's `mov r15,imm64` at
            // instruction start and the write part's after the read part (both
            // the machine symbol -- source and target elements share the machine
            // region). A FRAME-resident index on either side adds its own
            // frame-base `mov r10,imm64` relocation.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if *source_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_machine_indexed_frame_index_offset(
                        context.input.target.architecture,
                        *source_index_region,
                        false,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                    context.input.target.architecture,
                    *source_index_region,
                ),
                context.machine_storage_symbol_handle(),
            );
            if *target_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_machine_indexed_frame_index_offset(
                        context.input.target.architecture,
                        *source_index_region,
                        true,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::CopyRuntimeMachineDoubleIndexedToRuntimeStorage {
            outer_index_region,
            inner_index_region,
            target_region,
            ..
        } => {
            // Machine source base at instruction start; ONE shared frame base
            // (only when an index is frame-resident); the target-region base at
            // the write-half mov. The planner adds the +2 immediate offset.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if *outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                || *inner_index_region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
                    context.input.target.architecture,
                    *outer_index_region,
                    *inner_index_region,
                ),
                context.storage_region_symbol_handle(*target_region),
            );
            true
        }
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeMachineDoubleIndexed {
            source_region,
            outer_index_region,
            inner_index_region,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if [source_region, outer_index_region, inner_index_region].iter().any(|region| {
                **region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            }) {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::WriteRuntimeMachineDoubleIndexedInteger {
            outer_index_region,
            inner_index_region,
            ..
        } => {
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if [outer_index_region, inner_index_region].iter().any(|region| {
                **region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
            }) {
                context.insert_data_address_at_relative_offset(
                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::CopyRuntimeFrameBaseDoubleIndexedToRuntimeStorage {
            target_region,
            ..
        } => {
            // ONE frame-base relocation serves the array and both indices;
            // the target-region base relocates at the pre-store mov.
            context.insert_data_address_at_instruction_start(context.runtime_frame_symbol_handle());
            context.insert_data_address_at_relative_offset(
                runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
                    context.input.target.architecture,
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
