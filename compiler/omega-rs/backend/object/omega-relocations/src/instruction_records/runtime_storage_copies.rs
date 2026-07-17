use super::super::offsets::{
    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset,
    runtime_storage_copy_from_runtime_machine_indexed_target_address_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_frame_base_offset,
    runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset,
    runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset,
    runtime_storage_copy_machine_indexed_frame_index_offset,
    runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset,
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
        SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
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
                            source, target, *byte_count,
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
                        omega_instruction_selection::CopyPlacesShape::PointeePair { .. } => {
                            // Both pointer slots are frame-resident (the
                            // decompose's precondition); the retired
                            // fixed-indexed-to-pointee encoder reuses ONE
                            // frame base for both derefs -- the start
                            // relocation above is the only site.
                        }
                        omega_instruction_selection::CopyPlacesShape::FromIndexed {
                            element_byte_size,
                            field_byte_offset,
                            ..
                        } => {
                            // Frame-to-frame reuses the one frame base; a
                            // MACHINE target reloads its own base at the
                            // retired to-storage offset.
                            if target.region
                                == omega_target_operations::RuntimeStorageRegion::Machine
                            {
                                context.insert_data_address_at_relative_offset(
                                    runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                                        context.input.target.architecture,
                                        element_byte_size,
                                        field_byte_offset,
                                    ),
                                    context.storage_region_symbol_handle(target.region),
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
                        omega_instruction_selection::CopyPlacesShape::FromMachineIndexed {
                            base_byte_offset,
                            index_region,
                            index_offset,
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
        _ => false,
    }
}
