use super::super::offsets::{
    runtime_frame_base_indexed_string_data_address_offset,
    runtime_frame_indexed_string_data_address_offset,
    runtime_frame_indexed_string_data_address_offset_with_index_region,
    runtime_machine_double_indexed_string_data_address_offset,
    runtime_machine_indexed_string_data_address_offset_with_index_region,
    runtime_machine_indexed_string_runtime_frame_address_offset,
    string_descriptor_machine_address_offset, string_descriptor_pointee_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target::Architecture;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_string_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WritePlaceString {
            target,
            data,
            byte_length,
        } => {
            let data_symbol = context.data_object_symbol_handle(*data);
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    // Text rung 2a: the data reloc rides the leading
                    // `mov r14, imm64` at the instruction start; the target
                    // base + index bases patch BY PLACE REGION from the
                    // materializer's own sites (the WritePlaceInteger
                    // discipline).
                    context.insert_data_address_at_instruction_start(data_symbol);
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_write_place_string_with_sites(
                            target,
                            *byte_length,
                        )
                        .expect(
                            "WritePlaceString reached relocation with a shape the \
                             materializer refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a target ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a TargetIndex2 site implies two target ScaledIndex steps"),
                            omega_instruction_selection::PlaceCopySide::Source
                            | omega_instruction_selection::PlaceCopySide::SourceIndex
                            | omega_instruction_selection::PlaceCopySide::SourceIndex2 => {
                                unreachable!("a string write materializes only the target side")
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                Architecture::Aarch64 => {
                    // The transitional decompose: the SAME classifier the
                    // encoder uses picks the retained shape, so the relocs
                    // always describe the emitted bytes.
                    match omega_instruction_selection::classify_write_place_shape(target) {
                        omega_instruction_selection::WritePlaceShape::Direct { .. } => {
                            context.insert_data_address_at_instruction_start(data_symbol);
                            context.insert_data_address_at_relative_offset(
                                string_descriptor_machine_address_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::Pointee { .. } => {
                            context.insert_data_address_at_instruction_start(data_symbol);
                            context.insert_data_address_at_relative_offset(
                                string_descriptor_pointee_address_offset(
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
                                runtime_frame_indexed_string_data_address_offset(
                                    context.input.target.architecture,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                data_symbol,
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion {
                            index_region,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(target.region),
                            );
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.storage_region_symbol_handle(index_region),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_frame_indexed_string_data_address_offset_with_index_region(
                                    context.input.target.architecture,
                                    index_region,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                data_symbol,
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                            base_byte_offset,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(target.region),
                            );
                            context.insert_data_address_at_relative_offset(
                                runtime_frame_base_indexed_string_data_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                data_symbol,
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::MachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(target.region),
                            );
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_machine_indexed_string_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_machine_indexed_string_data_address_offset_with_index_region(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                data_symbol,
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(target.region),
                            );
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
                            context.insert_data_address_at_relative_offset(
                                runtime_machine_double_indexed_string_data_address_offset(
                                    context.input.target.architecture,
                                    outer_index_region,
                                    inner_index_region,
                                ),
                                data_symbol,
                            );
                        }
                        _ => unreachable!(
                            "an unsupported WritePlaceString shape refuses at aarch64 \
                             encoding; layout would have failed first"
                        ),
                    }
                }
            }
            true
        }
        SelectedInstructionKind::WritePlaceBoundedBuffer { target, literal } => {
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    // Content is immediate: the walk's base site(s) are the
                    // ONLY relocations, patched by place region from the
                    // materializer's own sites.
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_write_place_bounded_buffer_with_sites(
                            target, literal,
                        )
                        .expect(
                            "WritePlaceBoundedBuffer reached relocation with a shape the \
                             materializer refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a target ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a TargetIndex2 site implies two target ScaledIndex steps"),
                            omega_instruction_selection::PlaceCopySide::Source
                            | omega_instruction_selection::PlaceCopySide::SourceIndex
                            | omega_instruction_selection::PlaceCopySide::SourceIndex2 => {
                                unreachable!(
                                    "a bounded-buffer write materializes only the target side"
                                )
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                Architecture::Aarch64 => {
                    // The retained direct/pointee carrier encoders anchor
                    // their single base reloc at the instruction start.
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                }
            }
            true
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    let (_, sites) = omega_instruction_selection::x86_64_encode_append_place_bounded_buffer_source_with_sites(target, source)
                        .expect("bounded-buffer append layout would have refused before relocation");
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::Source => source.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("target index site implies an index"),
                            omega_instruction_selection::PlaceCopySide::SourceIndex => source
                                .scaled_index_region()
                                .expect("source index site implies an index"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("second target index site implies two indices"),
                            omega_instruction_selection::PlaceCopySide::SourceIndex2 => source
                                .scaled_index_regions()
                                .nth(1)
                                .expect("second source index site implies two indices"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                Architecture::Aarch64 => {
                    let (_, sites) = omega_instruction_selection::aarch64_encode_append_place_bounded_buffer_source_with_sites(target, source)
                        .expect("bounded-buffer append layout would have refused before relocation");
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::BoundedBufferPlaceSide::Target => {
                                target.region
                            }
                            omega_instruction_selection::BoundedBufferPlaceSide::Source => {
                                source.region
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
            }
            true
        }
        SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, literal } => {
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    let (_, sites) = omega_instruction_selection::x86_64_encode_append_place_bounded_buffer_literal_with_sites(target, literal)
                        .expect("bounded-buffer literal append layout would have refused before relocation");
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
                            _ => unreachable!("literal append materializes only its target Place"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                Architecture::Aarch64 => {
                    let (_, sites) = omega_instruction_selection::aarch64_encode_append_place_bounded_buffer_literal_with_sites(target, literal)
                        .expect("bounded-buffer literal append layout would have refused before relocation");
                    for (byte_offset, _) in sites.iter() {
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(target.region),
                        );
                    }
                }
            }
            true
        }
        _ => false,
    }
}
