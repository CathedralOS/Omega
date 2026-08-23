//! Maps retained place-pair and place-copy shapes to exact relocation address sites.

use super::*;

pub(super) fn compiler_place_pair_address_sites(
    architecture: Architecture,
    left: omega_target_operations::Place,
    right: omega_target_operations::Place,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    let omega_machine_bytes::CompilerInstructionValidationKind::PlacePairGuard {
        byte_size,
        failure_branch_distance,
        operator,
        is_float,
        ..
    } = kind
    else {
        return Err(Diagnostic::error(
            "invalid final place-pair validation recipe",
        ));
    };
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_place_compare(
                &left,
                &right,
                byte_size,
                failure_branch_distance,
                operator,
                is_float,
            )?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Source => left.region,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex => left
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-pair source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => left
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-pair second source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::Target => right.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => right
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-pair target index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => right
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-pair second target index relocation has no retained index step"))?,
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => Ok(vec![(0, left.region), (8, right.region)]),
    }
}

pub(super) fn compiler_place_copy_address_sites(
    architecture: Architecture,
    source: omega_target_operations::Place,
    target: omega_target_operations::Place,
    byte_count: usize,
) -> Result<Vec<(usize, omega_target_operations::RuntimeStorageRegion)>, Diagnostic> {
    match architecture {
        Architecture::X86_64 => {
            let (_, sites) = omega_isa_x86_64::encode_copy_places(&source, &target, byte_count)?;
            sites
                .iter()
                .map(|(offset, side)| {
                    let region = match side {
                        omega_isa_x86_64::PlaceCopySide::Source => source.region,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex => source
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-copy source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::SourceIndex2 => source
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-copy second source index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::Target => target.region,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex => target
                            .scaled_index_region()
                            .ok_or_else(|| Diagnostic::error("place-copy target index relocation has no retained index step"))?,
                        omega_isa_x86_64::PlaceCopySide::TargetIndex2 => target
                            .scaled_index_regions()
                            .nth(1)
                            .ok_or_else(|| Diagnostic::error("place-copy second target index relocation has no retained index step"))?,
                    };
                    Ok((offset, region))
                })
                .collect()
        }
        Architecture::Aarch64 => match compiler_body_place_copy_shape(&source, &target)? {
            CompilerBodyPlaceCopyShape::PointeePair { .. } => Ok(vec![(0, source.region)]),
            CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                let machine = omega_target_operations::RuntimeStorageRegion::Machine;
                if target.region == machine
                    || outer_index_region == machine
                    || inner_index_region == machine
                {
                    sites.push((32, machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromIndexed {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, index_region));
                }
                if target.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                            element_byte_size,
                            field_byte_offset,
                        ) + usize::from(index_region == omega_target_operations::RuntimeStorageRegion::Machine) * 8,
                        target.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToIndexed { .. }
            | CompilerBodyPlaceCopyShape::IndexedToPointee { .. }
            | CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                Ok(vec![(0, source.region)])
            }
            CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee { index_region, .. }
            | CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed { index_region, .. } => {
                let mut sites = vec![(0, source.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((12, index_region));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToFrameBaseIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_machine_index_base_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                } else if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_frame_base_indexed_operand_start_width_with_index_region(
                            base_byte_offset,
                            index_region,
                            index_offset,
                            index_byte_size,
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion { index_region, .. } => {
                Ok(vec![(0, source.region), (32, index_region)])
            }
            CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                index_region,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((32, index_region));
                } else if source.region == omega_target_operations::RuntimeStorageRegion::Machine {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
                            element_byte_size,
                            field_byte_offset,
                        ),
                        source.region,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromMachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    target.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToMachineIndexed {
                base_byte_offset,
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                            base_byte_offset,
                        ),
                        index_region,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_source_address_offset(
                        base_byte_offset,
                        index_region,
                        index_offset,
                        index_byte_size,
                        element_byte_size,
                        field_byte_offset,
                    ),
                    source.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::MachineIndexedToPointee { .. } => Ok(vec![
                (0, source.region),
                (
                    8,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceCopyShape::PointeeToMachineIndexed { .. } => Ok(vec![
                (0, target.region),
                (
                    8,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                    || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                {
                    sites.push((8, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_target_base_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    target.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                    || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                {
                    sites.push((12, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                    || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                {
                    sites.push((12, omega_target_operations::RuntimeStorageRegion::Machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee { .. } => Ok(vec![
                (0, source.region),
                (
                    8,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed { .. } => Ok(vec![
                (0, target.region),
                (
                    8,
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                ),
            ]),
            CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                if source.region == omega_target_operations::RuntimeStorageRegion::Machine
                    || outer_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                    || inner_index_region == omega_target_operations::RuntimeStorageRegion::Machine
                {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_source_base_offset(),
                        omega_target_operations::RuntimeStorageRegion::Machine,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                if outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    || inner_index_region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_target_base_offset(
                        outer_index_region,
                        inner_index_region,
                    ),
                    target.region,
                ));
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                outer_index_region,
                inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, target.region)];
                let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                if source.region == frame
                    || outer_index_region == frame
                    || inner_index_region == frame
                {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        frame,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::MachineIndexedPair {
                source_index_region,
                target_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                if source_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_machine_indexed_frame_index_offset(
                            source_index_region,
                            false,
                        ),
                        frame,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_second_base_offset(
                        source_index_region,
                    ),
                    target.region,
                ));
                if target_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_machine_indexed_frame_index_offset(
                            source_index_region,
                            true,
                        ),
                        frame,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair {
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
                ..
            } => {
                let frame = omega_target_operations::RuntimeStorageRegion::RuntimeFrame;
                let mut sites = vec![(0, source.region)];
                if source_outer_index_region == frame || source_inner_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_machine_double_indexed_frame_base_offset(),
                        frame,
                    ));
                }
                sites.push((
                    omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_pair_second_base_offset(
                        source_outer_index_region,
                        source_inner_index_region,
                    ),
                    target.region,
                ));
                if target_outer_index_region == frame || target_inner_index_region == frame {
                    sites.push((
                        omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_pair_target_frame_base_offset(
                            source_outer_index_region,
                            source_inner_index_region,
                        ),
                        frame,
                    ));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
                source_index_region,
                target_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                let machine = omega_target_operations::RuntimeStorageRegion::Machine;
                if source_index_region == machine || target_index_region == machine {
                    sites.push((12, machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::CrossRegionIndexedPair { .. } => {
                Ok(vec![(0, source.region), (8, target.region)])
            }
            CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair { .. } => {
                Ok(vec![(0, source.region), (8, target.region)])
            }
            CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
                ..
            } => {
                let mut sites = vec![(0, source.region)];
                let machine = omega_target_operations::RuntimeStorageRegion::Machine;
                if source_outer_index_region == machine
                    || source_inner_index_region == machine
                    || target_outer_index_region == machine
                    || target_inner_index_region == machine
                {
                    sites.push((12, machine));
                }
                Ok(sites)
            }
            CompilerBodyPlaceCopyShape::General => Err(Diagnostic::error(
                "final aarch64 place-copy relocation replay reached the x86-only general materializer class",
            )),
            _ => Ok(vec![(0, source.region), (8, target.region)]),
        },
    }
}
