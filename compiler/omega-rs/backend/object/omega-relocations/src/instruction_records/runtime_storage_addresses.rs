use super::context::InstructionRelocationContext;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_address_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } => {
            match context.input.target.architecture {
                omega_target::Architecture::X86_64 => {
                    // Task #131: the source base + index bases patch BY PLACE
                    // REGION from the materializer's own sites (the walk runs
                    // in the Target address register, so the SOURCE place's
                    // sites carry Target-side tags); the target frame slot's
                    // own `mov r14, imm64` sits at width-17 (mov 10 + store 7).
                    let (bytes, sites) =
                        omega_instruction_selection::x86_64_encode_write_place_address_with_sites(
                            source,
                            *target_offset,
                        )
                        .expect(
                            "WritePlaceAddress reached relocation with a shape the \
                             materializer refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Target => source.region,
                            omega_instruction_selection::PlaceCopySide::TargetIndex => source
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a source ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => source
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a TargetIndex2 site implies two source ScaledIndex steps"),
                            omega_instruction_selection::PlaceCopySide::Source
                            | omega_instruction_selection::PlaceCopySide::SourceIndex
                            | omega_instruction_selection::PlaceCopySide::SourceIndex2 => {
                                unreachable!(
                                    "an address write materializes only one place, walked \
                                     in the Target register"
                                )
                            }
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                    context.insert_data_address_at_relative_offset(
                        bytes.len() - 17,
                        context.runtime_frame_symbol_handle(),
                    );
                }
                omega_target::Architecture::Aarch64 => {
                    // The transitional decompose: the SAME classifier (plus the
                    // shared deref-indexed helper) the encoder uses picks the
                    // retained shape, so the relocs always describe the bytes.
                    let shape = omega_instruction_selection::classify_write_place_shape(source);
                    let frame_indexed =
                        omega_instruction_selection::classify_frame_base_indexed_address_shape(
                            source,
                        );
                    let frame_double_indexed =
                        omega_instruction_selection::classify_frame_base_double_indexed_address_shape(
                            source,
                        );
                    match shape {
                        omega_instruction_selection::WritePlaceShape::Direct { byte_offset } => {
                            context.insert_data_address_at_instruction_start(
                                context.storage_region_symbol_handle(source.region),
                            );
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::runtime_storage_address_to_runtime_frame_target_frame_offset(
                                    context.input.target.architecture,
                                    byte_offset,
                                ),
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. } => {
                            context.insert_data_address_at_instruction_start(
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                            ..
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.runtime_frame_symbol_handle(),
                            );
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
                                context.machine_storage_symbol_handle(),
                            );
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            context.insert_data_address_at_instruction_start(
                                context.machine_storage_symbol_handle(),
                            );
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::runtime_machine_double_indexed_address_frame_base_offset(
                                    context.input.target.architecture,
                                    outer_index_region,
                                    inner_index_region,
                                ),
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_indexed.is_some() =>
                        {
                            let frame_indexed = frame_indexed
                                .expect("guarded frame-base-indexed place-address source");
                            context.insert_data_address_at_instruction_start(
                                context.runtime_frame_symbol_handle(),
                            );
                            if frame_indexed.index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_frame_base_indexed_machine_index_base_offset(
                                        context.input.target.architecture,
                                        frame_indexed.base_byte_offset,
                                    ),
                                    context.machine_storage_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_double_indexed.is_some() =>
                        {
                            context.insert_data_address_at_instruction_start(
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        _ => {
                            if let Some((_, index_region, ..)) =
                                omega_instruction_selection::place_frame_deref_indexed_path(source)
                            {
                                // The deref-indexed shape (either index
                                // region): frame pair at the start; a
                                // machine-resident index materializes its
                                // page pair at the constant +32.
                                context.insert_data_address_at_instruction_start(
                                    context.runtime_frame_symbol_handle(),
                                );
                                if index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                                {
                                    context.insert_data_address_at_relative_offset(
                                        32,
                                        context.machine_storage_symbol_handle(),
                                    );
                                }
                            } else {
                                unreachable!(
                                    "an unsupported WritePlaceAddress shape refuses at \
                                     aarch64 encoding; layout would have failed first"
                                )
                            }
                        }
                    }
                }
            }
            true
        }
        _ => false,
    }
}
