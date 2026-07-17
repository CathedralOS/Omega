use super::super::offsets::{
    bounded_buffer_source_append_frame_address_offset,
    runtime_frame_indexed_string_data_address_offset,
    runtime_machine_indexed_string_data_address_offset,
    runtime_machine_indexed_string_runtime_frame_address_offset,
    string_descriptor_machine_address_offset, string_descriptor_pointee_address_offset,
    string_descriptor_runtime_frame_address_offset,
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
                        omega_instruction_selection::WritePlaceShape::MachineIndexed {
                            base_byte_offset,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(target.region),
                            );
                            context.insert_data_address_at_relative_offset(
                                runtime_machine_indexed_string_runtime_frame_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                ),
                                context.runtime_frame_symbol_handle(),
                            );
                            context.insert_data_address_at_relative_offset(
                                runtime_machine_indexed_string_data_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    element_byte_size,
                                    field_byte_offset,
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
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferSource {
            source_in_frame,
            ..
        } => {
            // The target carrier is machine-resident, addressed off the leading
            // base materialization. A frame-local source adds a second base
            // (the runtime frame) right after it -- `mov r14, imm64` on x86_64,
            // an `adrp`+`add` pair on aarch64 -- at the arch-aware offset.
            context.insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if *source_in_frame {
                context.insert_data_address_at_relative_offset(
                    bounded_buffer_source_append_frame_address_offset(
                        context.input.target.architecture,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            }
            true
        }
        SelectedInstructionKind::AppendRuntimeMachineBoundedBufferLiteral { .. } => {
            // The literal bytes are immediates; the target carrier is machine-
            // resident off the leading `mov r15, imm64` base -- the only reloc.
            context.insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            true
        }
        _ => false,
    }
}
