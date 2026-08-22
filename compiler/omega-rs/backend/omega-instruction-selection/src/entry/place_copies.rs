//! Compiler-body place-copy footprint derivation.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, validate_state_footprint,
};

/// Derive the target scratch footprint for the currently admitted ordinary
/// compiler-body place-copy subset. Direct storage pairs and direct-storage to
/// pointee copies are included; every other `CopyPlaces` shape remains outside
/// this partial evidence until its encoder publishes the corresponding
/// clobber contract.
pub fn derive_boundary_compiler_body_place_copy_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        } = instruction
        else {
            continue;
        };
        let clobbers = match (
            architecture,
            crate::classify_copy_places_shape(source, target),
        ) {
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::Direct { .. }) => {
                omega_isa_x86_64::copy_places_direct_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::Direct {
                    source_offset,
                    target_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_clobbers(
                source_offset,
                target_offset,
                *byte_count,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::ToPointee { .. }) => {
                omega_isa_x86_64::copy_places_to_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToPointee {
                    source_offset,
                    pointer_byte_offset,
                    field_byte_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                *byte_count,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::FromPointee { .. }) => {
                omega_isa_x86_64::copy_places_from_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromPointee {
                    pointer_byte_offset,
                    field_byte_offset,
                    target_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_clobbers(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                *byte_count,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromPointeeDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromPointeeDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_double_indexed_clobbers(
                target.region,
                outer_index_region,
                inner_index_region,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::PointeePair { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_pointee_pair_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeePair {
                    source_field_byte_offset,
                    target_field_byte_offset,
                    ..
                },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_pointee_pair_clobbers(
                    source_field_byte_offset,
                    target_field_byte_offset,
                    *byte_count,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::FromIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_from_indexed_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromIndexed { index_region, .. },
            )
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
                    index_region,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_to_indexed_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToIndexedByRegion { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (omega_target::Architecture::Aarch64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_clobbers()
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToIndexedByRegion { index_region, .. },
            ) if target.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
                    source.region,
                    index_region,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::IndexedToPointee { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_indexed_to_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::IndexedToPointeeByRegion { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::IndexedToPointee { .. },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers()
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::IndexedToPointeeByRegion { index_region, .. },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
                    index_region,
                )
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_frame_base_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromFrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToFrameBaseIndexed { index_region, .. },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_indexed_clobbers(
                source.region,
                index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToFrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_machine_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_to_machine_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_frame_base_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromFrameBaseDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToFrameBaseDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_clobbers(
                source.region,
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedToPointee { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_machine_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromMachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_to_machine_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToMachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_double_indexed_clobbers(
                source.region,
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_machine_indexed_pair_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseIndexedPair {
                    source_index_region,
                    target_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_clobbers(
                source_index_region,
                target_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::CrossRegionIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::CrossRegionIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_cross_region_indexed_pair_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::CrossRegionDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::CrossRegionDoubleIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_cross_region_double_indexed_pair_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedPair {
                    source_outer_index_region,
                    source_inner_index_region,
                    target_outer_index_region,
                    target_inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_clobbers(
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineDoubleIndexedPair {
                    source_outer_index_region,
                    source_inner_index_region,
                    target_outer_index_region,
                    target_inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_clobbers(
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineDoubleIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineDoubleIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToMachineDoubleIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_clobbers(),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::General) => {
                omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count)
            }
            _ => continue,
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}
