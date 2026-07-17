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
                    match omega_instruction_selection::classify_write_place_shape(source) {
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
                                    element_byte_size,
                                    field_byte_offset,
                                ),
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
        SelectedInstructionKind::WriteRuntimeMachineIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_region,
            index_offset,
            element_byte_size,
            field_byte_offset,
            ..
        } => {
            // MACHINE base (the element-address computation) at the start,
            // the index region's base at its own load, and the target frame
            // base at the store's reload.
            context
                .insert_data_address_at_instruction_start(context.machine_storage_symbol_handle());
            if context.input.target.architecture
                == omega_target::Architecture::Aarch64
            {
                // aarch64 shares the machine-indexed COPY family's address
                // prefix byte-for-byte, so its offset fns describe this
                // instruction too: a FRAME-resident index loads its own page
                // pair (a MACHINE index reads through the copied base -- no
                // second site), and the target frame pair sits right after
                // the address computation (the copy family's source-adrp
                // position).
                if *index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    context.insert_data_address_at_relative_offset(
                        omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                            context.input.target.architecture,
                            *base_byte_offset,
                        ),
                        context.runtime_frame_symbol_handle(),
                    );
                }
                context.insert_data_address_at_relative_offset(
                    omega_instruction_selection::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                        context.input.target.architecture,
                        *base_byte_offset,
                        *index_region,
                        *index_offset,
                        *element_byte_size,
                        *field_byte_offset,
                    ),
                    context.runtime_frame_symbol_handle(),
                );
            } else if let Some((index_base_offset, target_frame_offset)) =
                omega_instruction_selection::runtime_machine_indexed_address_relocation_offsets(
                    context.input.target.architecture,
                )
            {
                let index_symbol = context.storage_region_symbol_handle(*index_region);
                context.insert_data_address_at_relative_offset(index_base_offset, index_symbol);
                context.insert_data_address_at_relative_offset(
                    target_frame_offset,
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
