use super::super::offsets::{
    runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset,
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset,
    runtime_storage_copy_machine_indexed_frame_index_offset,
    runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset,
    runtime_storage_copy_target_address_offset,
    runtime_storage_copy_to_runtime_machine_indexed_source_address_offset,
};
use super::context::InstructionRelocationContext;
use omega_target::Architecture;
use omega_target_operations::SelectedInstructionKind;

pub(super) fn collect_runtime_storage_copy_relocations(
    context: &mut InstructionRelocationContext<'_, '_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    match instruction {
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            ..
        } => {
            // The rung-2 walker arm: patch BY PLACE REGION. On x86_64 the
            // materializer reports its base-mov sites from the SAME walk that
            // emitted the bytes (never a hand-maintained offset constant);
            // each site patches from the region of the place on its side. On
            // aarch64 the transitional direct-pair shape reuses the retired
            // plain-copy layout: source base at the start, target base at the
            // arch offset.
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_copy_places_with_sites(
                            source,
                            target,
                            *byte_count,
                        )
                        .expect(
                            "CopyPlaces reached relocation with a shape the materializer \
                             refuses; layout/encoding would have failed first",
                        );
                    for (byte_offset, side) in sites.iter() {
                        let region = match side {
                            omega_instruction_selection::PlaceCopySide::Source => source.region,
                            omega_instruction_selection::PlaceCopySide::Target => target.region,
                            omega_instruction_selection::PlaceCopySide::SourceIndex => source
                                .scaled_index_region()
                                .expect("a SourceIndex site implies a source ScaledIndex step"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex => target
                                .scaled_index_region()
                                .expect("a TargetIndex site implies a target ScaledIndex step"),
                            // The double-index rung: the SECOND ScaledIndex
                            // step's own region, in walk order.
                            omega_instruction_selection::PlaceCopySide::SourceIndex2 => source
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a SourceIndex2 site implies two source ScaledIndex steps"),
                            omega_instruction_selection::PlaceCopySide::TargetIndex2 => target
                                .scaled_index_regions()
                                .nth(1)
                                .expect("a TargetIndex2 site implies two target ScaledIndex steps"),
                        };
                        context.insert_data_address_at_relative_offset(
                            byte_offset,
                            context.storage_region_symbol_handle(region),
                        );
                    }
                }
                Architecture::Aarch64 => {
                    // The transitional decompose: the SAME classifier the
                    // encoder uses picks the retired shape, so the reloc
                    // offsets below always describe the bytes actually
                    // emitted. Every retired layout anchors its FIRST base
                    // at the instruction start -- the source side for all
                    // shapes except the machine-array WRITE, whose layout
                    // opens with the machine (target) base for the index
                    // address setup.
                    let shape =
                        omega_instruction_selection::classify_copy_places_shape(source, target);
                    let start_region = match shape {
                        omega_instruction_selection::CopyPlacesShape::ToMachineIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::ToFrameBaseIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::ToMachineDoubleIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::ToFrameBaseDoubleIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::ToIndexedByRegion {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToMachineDoubleIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToMachineIndexed {
                            ..
                        } => target.region,
                        _ => source.region,
                    };
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(start_region),
                    );
                    match shape {
                        omega_instruction_selection::CopyPlacesShape::Direct { .. }
                        | omega_instruction_selection::CopyPlacesShape::ToPointee { .. } => {
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_target_address_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::FromPointee { .. } => {
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_from_runtime_pointee_to_runtime_frame_target_address_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::FromPointeeDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            if target.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                                || outer_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.machine_storage_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::PointeePair { .. } => {
                            // Both pointer slots are frame-resident (the
                            // decompose's precondition); the retired
                            // fixed-indexed-to-pointee encoder reuses ONE
                            // frame base for both derefs -- the start
                            // relocation above is the only site.
                        }
                        omega_instruction_selection::CopyPlacesShape::FromIndexed {
                            index_region,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
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
                            // Frame-to-frame reuses the descriptor base for
                            // its target; a MACHINE target reloads its own
                            // base after the complete indexed-source setup.
                            if target.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                                        context.input.target.architecture,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8,
                                    context.storage_region_symbol_handle(target.region),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::ToIndexedByRegion {
                            index_region,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.storage_region_symbol_handle(index_region),
                                );
                            } else if source.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                                        context.input.target.architecture,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    ),
                                    context.storage_region_symbol_handle(source.region),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::ToIndexed { .. }
                        | omega_instruction_selection::CopyPlacesShape::IndexedToPointee {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::FromFrameBaseIndexed {
                            ..
                        } => {
                            // Frame-rooted on both sides (the decompose's
                            // precondition): one frame base serves the
                            // array/descriptor, index, and the other side.
                        }
                        omega_instruction_selection::CopyPlacesShape::FrameBaseIndexedPair {
                            source_index_region,
                            target_index_region,
                            ..
                        } => {
                            let machine =
                                omega_target_operations::RuntimeStorageRegion::Machine;
                            if source_index_region == machine || target_index_region == machine {
                                context.insert_data_address_at_relative_offset(
                                    12,
                                    context.storage_region_symbol_handle(machine),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::CrossRegionIndexedPair {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::CrossRegionDoubleIndexedPair {
                            ..
                        } => {
                            context.insert_data_address_at_relative_offset(
                                8,
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::FrameBaseDoubleIndexedPair {
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                            ..
                        } => {
                            let machine =
                                omega_target_operations::RuntimeStorageRegion::Machine;
                            if source_outer_index_region == machine
                                || source_inner_index_region == machine
                                || target_outer_index_region == machine
                                || target_inner_index_region == machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    12,
                                    context.storage_region_symbol_handle(machine),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::FrameBaseDoubleIndexedToPointee {
                            outer_index_region,
                            inner_index_region,
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    12,
                                    context.storage_region_symbol_handle(
                                        omega_target_operations::RuntimeStorageRegion::Machine,
                                    ),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::FrameBaseIndexedToPointee {
                            index_region, ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToFrameBaseIndexed {
                            index_region, ..
                        } => {
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    12,
                                    context.storage_region_symbol_handle(index_region),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::MachineDoubleIndexedToPointee {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToMachineDoubleIndexed {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::MachineIndexedToPointee {
                            ..
                        }
                        | omega_instruction_selection::CopyPlacesShape::PointeeToMachineIndexed {
                            ..
                        } => {
                            context.insert_data_address_at_relative_offset(
                                8,
                                context.runtime_frame_symbol_handle(),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::ToFrameBaseIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_frame_base_indexed_machine_index_base_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.storage_region_symbol_handle(index_region),
                                );
                            } else if source.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_frame_base_indexed_operand_start_width_with_index_region(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                        index_region,
                                        index_offset,
                                        index_byte_size,
                                        element_byte_size,
                                        field_byte_offset,
                                    ),
                                    context.storage_region_symbol_handle(source.region),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::IndexedToPointeeByRegion {
                            index_region,
                            ..
                        } => {
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                    context.input.target.architecture,
                                ),
                                context.storage_region_symbol_handle(index_region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::FromMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            // Mirrors the variant's own arm: machine source
                            // base at start (above); ONE shared frame base
                            // when an index is frame-resident; the
                            // target-region base at the write-half mov.
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                || inner_index_region
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
                                    outer_index_region,
                                    inner_index_region,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::FromFrameBaseDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            // The start frame root serves the array and any
                            // frame indices. One machine root serves either
                            // or both machine indices before the address walk.
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    8,
                                    context.storage_region_symbol_handle(
                                        omega_target_operations::RuntimeStorageRegion::Machine,
                                    ),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
                                    context.input.target.architecture,
                                    outer_index_region,
                                    inner_index_region,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::ToFrameBaseDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            // The target frame root at the start also serves
                            // frame indices and a frame source. One machine
                            // root serves the source and/or either machine
                            // index after the frame-base preservation move.
                            if source.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                                || outer_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset(),
                                    context.storage_region_symbol_handle(
                                        omega_target_operations::RuntimeStorageRegion::Machine,
                                    ),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::ToMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            // Mirrors the variant's own arm: machine (target)
                            // base at start (above); ONE shared frame base
                            // when the source or an index is frame-resident.
                            if source.region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                || outer_index_region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::MachineIndexedPair {
                            source_index_region,
                            target_index_region,
                            ..
                        } => {
                            // Mirrors the variant's own arm: machine (read)
                            // base at start; frame-resident indices add their
                            // frame base per side; the write part reloads the
                            // machine base at the second-base offset.
                            if source_index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_machine_indexed_frame_index_offset(
                                        context.input.target.architecture,
                                        source_index_region,
                                        false,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                                    context.input.target.architecture,
                                    source_index_region,
                                ),
                                context.machine_storage_symbol_handle(),
                            );
                            if target_index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_machine_indexed_frame_index_offset(
                                        context.input.target.architecture,
                                        source_index_region,
                                        true,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::MachineDoubleIndexedPair {
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                            ..
                        } => {
                            let frame =
                                omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                            if source_outer_index_region == frame
                                || source_inner_index_region == frame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::aarch64_runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
                                    source_outer_index_region,
                                    source_inner_index_region,
                                ),
                                context.machine_storage_symbol_handle(),
                            );
                            if target_outer_index_region == frame
                                || target_inner_index_region == frame
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::aarch64_runtime_storage_copy_machine_double_indexed_pair_target_frame_base_offset(
                                        source_outer_index_region,
                                        source_inner_index_region,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::CopyPlacesShape::FromMachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            // Machine base at start (above); a frame-resident
                            // index reloads the frame base mid-sequence; the
                            // target base at the shape's offset fn.
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                    *byte_count,
                                ),
                                context.storage_region_symbol_handle(target.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::ToMachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            // Machine base at start (above); a frame-resident
                            // index adds its frame page-pair at the same
                            // offset as the read layout; the SOURCE base at
                            // the write shape's offset fn.
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                            context.insert_data_address_at_relative_offset(
                                runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                                    context.input.target.architecture,
                                    base_byte_offset,
                                    index_region,
                                    index_offset,
                                    index_byte_size,
                                    element_byte_size,
                                    field_byte_offset,
                                ),
                                context.storage_region_symbol_handle(source.region),
                            );
                        }
                        omega_instruction_selection::CopyPlacesShape::General => {
                            unreachable!(
                                "CopyPlaces General shape reached aarch64 relocation; \
                                 layout/encoding refuses it first"
                            );
                        }
                    }
                }
            }
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
        SelectedInstructionKind::WritePlaceInteger {
            target,
            value,
            byte_size,
        } => {
            // Write rung 2a: patch BY PLACE REGION from the materializer's
            // own sites (the CopyPlaces x86 arm's discipline). aarch64 never
            // reaches here -- its encoding refuses until the decompose rung.
            match context.input.target.architecture {
                Architecture::X86_64 => {
                    let (_, sites) =
                        omega_instruction_selection::x86_64_encode_write_place_integer_with_sites(
                            target, *value, *byte_size,
                        )
                        .expect(
                            "WritePlaceInteger reached relocation with a shape the                              materializer refuses; layout/encoding would have failed first",
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
                                unreachable!("an integer write materializes only the target side")
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
                    // always describe the emitted bytes. Every retained
                    // integer-write layout anchors its base at the
                    // instruction start; the machine-indexed shapes add
                    // their frame-index relocations.
                    let shape = omega_instruction_selection::classify_write_place_shape(target);
                    let frame_indexed =
                        omega_instruction_selection::classify_frame_base_indexed_integer_shape(
                            target,
                        );
                    let frame_double =
                        omega_instruction_selection::classify_frame_base_double_indexed_integer_shape(
                            target,
                        );
                    context.insert_data_address_at_instruction_start(
                        context.storage_region_symbol_handle(target.region),
                    );
                    match shape {
                        omega_instruction_selection::WritePlaceShape::Direct { .. }
                        | omega_instruction_selection::WritePlaceShape::Pointee { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameIndexed { .. }
                        | omega_instruction_selection::WritePlaceShape::FrameBaseIndexed {
                            ..
                        } => {}
                        omega_instruction_selection::WritePlaceShape::FrameIndexedByRegion {
                            index_region,
                            ..
                        } => {
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
                        }
                        omega_instruction_selection::WritePlaceShape::MachineIndexed {
                            base_byte_offset,
                            index_region,
                            ..
                        } => {
                            if index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::runtime_machine_indexed_integer_runtime_frame_address_offset(
                                        context.input.target.architecture,
                                        base_byte_offset,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::WritePlaceShape::MachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            // aarch64 keeps its retired shared-base layout.
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.runtime_frame_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::WritePlaceShape::PointeeDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            if outer_index_region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                                || inner_index_region
                                    == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    omega_instruction_selection::frame_indexed_operand_machine_index_base_offset(
                                        context.input.target.architecture,
                                    ),
                                    context.machine_storage_symbol_handle(),
                                );
                            }
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_indexed.is_some() =>
                        {
                            let frame_indexed = frame_indexed
                                .expect("the guarded frame-base-indexed shape must be retained");
                            context.insert_data_address_at_relative_offset(
                                omega_instruction_selection::runtime_frame_base_indexed_machine_index_base_offset(
                                    context.input.target.architecture,
                                    frame_indexed.base_byte_offset,
                                ),
                                context.storage_region_symbol_handle(frame_indexed.index_region),
                            );
                        }
                        omega_instruction_selection::WritePlaceShape::Unsupported
                            if frame_double.is_some() => {}
                        omega_instruction_selection::WritePlaceShape::Unsupported => {
                            unreachable!(
                                "an unsupported WritePlaceInteger shape refuses at \
                                 aarch64 encoding; layout would have failed first"
                            )
                        }
                    }
                }
            }
            true
        }
        _ => false,
    }
}
