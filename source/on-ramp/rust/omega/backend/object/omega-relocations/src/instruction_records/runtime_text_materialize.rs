use super::super::offsets::{
    runtime_text_buffer_materialize_target_address_offset,
    runtime_text_indexed_buffer_materialize_buffer_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_materialize_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        // Task #132: the place-shaped survivor decomposes by the SAME
        // classifier the encoder uses -- direct/pointee share the buffer@0 +
        // target-at-offset layout (the pointee target base is the frame);
        // frame-indexed flips to frame@0 + buffer at the indexed offset.
        SelectedInstructionKind::MaterializeTextBufferToPlace { buffer, target } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            match (
                context.input.target.architecture,
                omega_instruction_selection::classify_write_place_shape(target),
            ) {
                (
                    _,
                    omega_instruction_selection::WritePlaceShape::Direct { .. }
                    | omega_instruction_selection::WritePlaceShape::Pointee { .. },
                ) => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_buffer_materialize_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                }
                (
                    omega_target::Architecture::X86_64,
                    omega_instruction_selection::WritePlaceShape::FrameIndexed {
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                        ..
                    },
                ) => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_buffer_materialize_buffer_address_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                }
                (omega_target::Architecture::X86_64, _) => {
                    let (_, sites, buffer_site) = omega_instruction_selection::x86_64_encode_runtime_text_buffer_materialize_to_place_with_sites(target)
                        .expect("general x86 text materialization reached relocation after successful layout");
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("target index site implies an index"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("second target index site implies two indices"),
                            _ => unreachable!("text materialization walks only its target"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    context.insert_data_address_at_relative_offset(buffer_site, buffer_symbol);
                }
                (
                    omega_target::Architecture::Aarch64,
                    omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion { .. }
                    | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed { .. },
                ) => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    match omega_instruction_selection::classify_write_place_shape(target) {
                        omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion {
                            index_region,
                            ..
                        } if index_region
                            == omega_target_operations::RuntimeStorageRegion::Machine =>
                        {
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(index_region),
                            );
                        }
                        _ => {}
                    }
                    let buffer_site = omega_instruction_selection::runtime_text_buffer_materialize_to_place_width(
                        context.input.target.architecture,
                        target,
                    )
                    .checked_sub(40)
                    .expect("aarch64 indexed text-buffer materialization includes its fixed tail");
                    context.insert_data_address_at_relative_offset(buffer_site, buffer_symbol);
                }
                (
                    omega_target::Architecture::Aarch64,
                    omega_instruction_selection::WritePlaceShape::Unsupported,
                ) if omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_double_indexed_materialize_buffer_address_offset(),
                        buffer_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported MaterializeTextBufferToPlace shape refuses at \
                     encoding; layout would have failed first"
                ),
            }
            true
        }
        _ => false,
    }
}
