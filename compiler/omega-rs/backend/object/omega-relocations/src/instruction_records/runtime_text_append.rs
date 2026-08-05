use super::super::offsets::{
    runtime_text_indexed_literal_append_buffer_address_offset,
    runtime_text_indexed_stored_place_buffer_address_offset,
    runtime_text_indexed_stored_place_source_address_offset,
    runtime_text_literal_append_target_address_offset,
    runtime_text_stored_place_pointee_source_address_offset,
    runtime_text_stored_place_source_address_offset,
    runtime_text_stored_place_target_address_offset,
    runtime_text_stored_suffix_source_address_offset,
    runtime_text_stored_suffix_target_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_text_append_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer,
            source_region,
            target_region,
            ..
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            let target_symbol = context.storage_region_symbol_handle(*target_region);
            context.insert_data_address_at_instruction_start(buffer_symbol);
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_source_address_offset(context.input.target.architecture),
                source_symbol,
            );
            context.insert_data_address_at_relative_offset(
                runtime_text_stored_suffix_target_address_offset(context.input.target.architecture),
                target_symbol,
            );
            true
        }
        // Task #132: the place-shaped survivors decompose by shape.
        SelectedInstructionKind::AppendTextStoredToPlace {
            buffer,
            source_region,
            source_offset,
            target,
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            let source_symbol = context.storage_region_symbol_handle(*source_region);
            match omega_instruction_selection::classify_write_place_shape(target) {
                omega_instruction_selection::WritePlaceShape::Direct { .. } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_source_address_offset(
                            context.input.target.architecture,
                        ),
                        source_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::Pointee {
                    pointer_byte_offset,
                    field_byte_offset,
                } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_stored_place_pointee_source_address_offset(
                            context.input.target.architecture,
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        source_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::FrameIndexed {
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                    ..
                } => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_stored_place_buffer_address_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_stored_place_source_address_offset(
                            context.input.target.architecture,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source_symbol,
                    );
                }
                _ if context.input.target.architecture == omega_target::Architecture::X86_64 => {
                    let (_, sites, buffer_site, source_site) = omega_instruction_selection::x86_64_encode_runtime_text_stored_append_to_place_with_sites(target, *source_offset)
                        .expect("general x86 stored-text append reached relocation after successful layout");
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
                            _ => unreachable!("stored-text append walks only its target"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    context.insert_data_address_at_relative_offset(buffer_site, buffer_symbol);
                    context.insert_data_address_at_relative_offset(source_site, source_symbol);
                }
                omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } if context.input.target.architecture == omega_target::Architecture::Aarch64 => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_indexed_stored_place_buffer_address_offset(
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_indexed_stored_place_source_address_offset(
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::Unsupported
                    if context.input.target.architecture == omega_target::Architecture::Aarch64
                        && omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() =>
                {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_double_indexed_stored_place_buffer_address_offset(),
                        buffer_symbol,
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_double_indexed_stored_place_source_address_offset(),
                        source_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported AppendTextStoredToPlace shape refuses at encoding; \
                     layout would have failed first"
                ),
            }
            true
        }
        SelectedInstructionKind::AppendTextLiteralToPlace {
            buffer,
            target,
            literal,
        } => {
            let buffer_symbol = context.data_object_symbol_handle(*buffer);
            match omega_instruction_selection::classify_write_place_shape(target) {
                omega_instruction_selection::WritePlaceShape::Direct { .. }
                | omega_instruction_selection::WritePlaceShape::Pointee { .. } => {
                    context.insert_data_address_at_instruction_start(buffer_symbol);
                    context.insert_data_address_at_relative_offset(
                        runtime_text_literal_append_target_address_offset(
                            context.input.target.architecture,
                        ),
                        context.storage_region_symbol_handle(target.region),
                    );
                }
                omega_instruction_selection::WritePlaceShape::FrameIndexed {
                    element_byte_size,
                    field_byte_offset,
                    ..
                } => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        runtime_text_indexed_literal_append_buffer_address_offset(
                            context.input.target.architecture,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                }
                _ if context.input.target.architecture == omega_target::Architecture::X86_64 => {
                    let (_, sites, buffer_site) = omega_instruction_selection::x86_64_encode_runtime_text_literal_append_to_place_with_sites(target, literal)
                        .expect("general x86 literal append reached relocation after successful layout");
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
                            _ => unreachable!("literal append walks only its target"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    context.insert_data_address_at_relative_offset(buffer_site, buffer_symbol);
                }
                omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                    base_byte_offset,
                    index_offset,
                    index_byte_size,
                    element_byte_size,
                    field_byte_offset,
                } if context.input.target.architecture == omega_target::Architecture::Aarch64 => {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_indexed_literal_append_buffer_address_offset(
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        buffer_symbol,
                    );
                }
                omega_instruction_selection::WritePlaceShape::Unsupported
                    if context.input.target.architecture == omega_target::Architecture::Aarch64
                        && omega_instruction_selection::classify_frame_base_double_indexed_text_assembly_shape(target).is_some() =>
                {
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_text_frame_base_double_indexed_literal_append_buffer_address_offset(),
                        buffer_symbol,
                    );
                }
                _ => unreachable!(
                    "an unsupported AppendTextLiteralToPlace shape refuses at encoding; \
                     layout would have failed first"
                ),
            }
            true
        }
        _ => false,
    }
}
