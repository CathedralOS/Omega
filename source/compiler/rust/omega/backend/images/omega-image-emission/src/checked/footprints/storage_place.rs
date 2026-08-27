//! Derives compiler atomic, place, storage-result, arithmetic, and conversion footprints.

use super::*;

fn compiler_atomic_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    operation: omega_machine_bytes::CompilerInstructionAtomicOperation,
) -> (
    omega_calling_conventions::RegisterSet,
    omega_calling_conventions::MachineStateSet,
) {
    use omega_calling_conventions::{MachineRegister, MachineState, MachineStateSet, RegisterSet};
    use omega_machine_bytes::CompilerInstructionAtomicOperation;

    if matches!(operation, CompilerInstructionAtomicOperation::Load { .. }) {
        return match architecture {
            Architecture::X86_64 => (
                RegisterSet::new([MachineRegister::X86R10, MachineRegister::X86R14]),
                MachineStateSet::empty(),
            ),
            Architecture::Aarch64 => (
                RegisterSet::new([MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17)]),
                MachineStateSet::empty(),
            ),
        };
    }

    let (operands, writes_flags, writes_stack) = match operation {
        CompilerInstructionAtomicOperation::Store { value, .. }
        | CompilerInstructionAtomicOperation::Swap {
            new_value: value, ..
        } => (vec![value], false, false),
        CompilerInstructionAtomicOperation::FetchXor { value, .. }
        | CompilerInstructionAtomicOperation::FetchOr { value, .. }
        | CompilerInstructionAtomicOperation::FetchAnd { value, .. } => (vec![value], true, false),
        CompilerInstructionAtomicOperation::FetchAdd { delta, .. }
        | CompilerInstructionAtomicOperation::FetchSub { delta, .. } => (vec![delta], true, false),
        CompilerInstructionAtomicOperation::CompareExchange {
            expected,
            new_value,
            ..
        } => (vec![new_value, expected], true, true),
        CompilerInstructionAtomicOperation::Load { .. } => unreachable!("handled above"),
    };
    let (registers, state) = match architecture {
        Architecture::X86_64 => {
            let mut state = MachineStateSet::empty();
            for operand in operands {
                state = state.union(
                    omega_isa_x86_64::runtime_value_operand_additional_machine_state(
                        runtime_value_operands,
                        operand,
                    ),
                );
            }
            if writes_flags {
                state = state.union(MachineStateSet::new([MachineState::Flags]));
            }
            if writes_stack {
                state = state.union(MachineStateSet::new([MachineState::StackPointer]));
            }
            (
                omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                state,
            )
        }
        Architecture::Aarch64 => {
            let mut state = MachineStateSet::empty();
            for operand in operands {
                state = state.union(
                    omega_isa_aarch64::runtime_value_operand_additional_machine_state(
                        runtime_value_operands,
                        operand,
                    ),
                );
            }
            (
                omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                state,
            )
        }
    };
    (registers, state)
}

pub(super) fn storage_place_footprint_parts(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprintParts> {
    use omega_calling_conventions::MachineStateSet;
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let parts = match kind {
        CompilerInstructionValidationKind::CompilerBodyAtomic(operation) => {
            let (registers, additional_state) =
                compiler_atomic_footprint(architecture, runtime_value_operands, operation);
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::ExitIndirectResultCopy {
            source,
            target,
            byte_count,
        } => {
            let Ok((source_offset, pointer_byte_offset)) =
                compiler_exit_indirect_result_copy_offsets(&source, &target)
            else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy,
                match architecture {
                    Architecture::X86_64 => {
                        omega_isa_x86_64::copy_places_to_pointee_clobbers(byte_count)
                    }
                    Architecture::Aarch64 => {
                        omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                            source_offset,
                            pointer_byte_offset,
                            0,
                            byte_count,
                        )
                    }
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceCopy {
            source,
            target,
            byte_count,
        } => {
            let Ok(shape) = compiler_body_place_copy_shape(&source, &target) else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy,
                match architecture {
                    Architecture::X86_64 => match shape {
                        CompilerBodyPlaceCopyShape::Direct { .. } => {
                            omega_isa_x86_64::copy_places_direct_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToPointee { .. } => {
                            omega_isa_x86_64::copy_places_to_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromPointee { .. } => {
                            omega_isa_x86_64::copy_places_from_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::PointeePair { .. } => {
                            omega_isa_x86_64::copy_places_pointee_pair_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToIndexedByRegion { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointee { .. } => {
                            omega_isa_x86_64::copy_places_indexed_to_pointee_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_frame_base_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee { .. }
                        | CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromMachineIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_machine_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToMachineIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_machine_indexed_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::MachineIndexedToPointee { .. }
                        | CompilerBodyPlaceCopyShape::PointeeToMachineIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_frame_base_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee { .. }
                        | CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_from_machine_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed { .. } => {
                            omega_isa_x86_64::copy_places_to_machine_double_indexed_clobbers(
                                byte_count,
                            )
                        }
                        CompilerBodyPlaceCopyShape::MachineIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_machine_indexed_pair_clobbers(byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::CrossRegionIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair { .. } => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                        CompilerBodyPlaceCopyShape::General => {
                            omega_isa_x86_64::copy_places_clobbers(&source, &target, byte_count)
                        }
                    },
                    Architecture::Aarch64 => match shape {
                        CompilerBodyPlaceCopyShape::Direct {
                            source_offset,
                            target_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_clobbers(
                            source_offset,
                            target_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::ToPointee {
                            source_offset,
                            pointer_byte_offset,
                            field_byte_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                            source_offset,
                            pointer_byte_offset,
                            field_byte_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::FromPointee {
                            pointer_byte_offset,
                            field_byte_offset,
                            target_offset,
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_clobbers(
                            pointer_byte_offset,
                            field_byte_offset,
                            target_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::FromPointeeDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_double_indexed_clobbers(
                            target.region,
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::PointeePair {
                            source_field_byte_offset,
                            target_field_byte_offset,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_pointee_pair_clobbers(
                            source_field_byte_offset,
                            target_field_byte_offset,
                            byte_count,
                        ),
                        CompilerBodyPlaceCopyShape::FromIndexed { index_region, .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
                                index_region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::ToIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToIndexedByRegion {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
                            source.region,
                            index_region,
                        ),
                        CompilerBodyPlaceCopyShape::IndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::IndexedToPointeeByRegion {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
                            index_region,
                        ),
                        CompilerBodyPlaceCopyShape::FromFrameBaseIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseIndexed { index_region, .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_indexed_clobbers(
                                source.region,
                                index_region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseIndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::PointeeToFrameBaseIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::FromMachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToMachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::MachineIndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::PointeeToMachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::FromFrameBaseDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers(
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::PointeeToFrameBaseDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::MachineDoubleIndexedToPointee { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::PointeeToMachineDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::ToFrameBaseDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_clobbers(
                                source.region,
                                outer_index_region,
                                inner_index_region,
                            )
                        }
                        CompilerBodyPlaceCopyShape::FromMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::ToMachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_double_indexed_clobbers(
                            source.region,
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::MachineIndexedPair { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseIndexedPair {
                            source_index_region,
                            target_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_clobbers(
                            source_index_region,
                            target_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::CrossRegionIndexedPair { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_cross_region_indexed_pair_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::CrossRegionDoubleIndexedPair { .. } => {
                            omega_isa_aarch64::runtime_storage_copy_cross_region_double_indexed_pair_clobbers()
                        }
                        CompilerBodyPlaceCopyShape::FrameBaseDoubleIndexedPair {
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_clobbers(
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::MachineDoubleIndexedPair {
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_clobbers(
                            source_outer_index_region,
                            source_inner_index_region,
                            target_outer_index_region,
                            target_inner_index_region,
                        ),
                        CompilerBodyPlaceCopyShape::General => return None,
                    },
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceIntegerWrite { target, .. } => {
            let Ok(shape) = compiler_body_place_write_shape_with_cross_region_frame_base(&target)
            else {
                return None;
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
                match architecture {
                    Architecture::X86_64 => omega_isa_x86_64::place_integer_write_clobbers(&target),
                    Architecture::Aarch64 => match shape {
                        CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset } => {
                            omega_isa_aarch64::runtime_machine_integer_write_clobbers(byte_offset)
                        }
                        CompilerBodyPlaceIntegerWriteShape::Pointee {
                            pointer_byte_offset,
                            field_byte_offset,
                        } => omega_isa_aarch64::runtime_pointee_integer_write_clobbers(
                            pointer_byte_offset,
                            field_byte_offset,
                        ),
                        CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
                            index_region, ..
                        } => omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(
                            index_region,
                        ),
                        CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
                            index_region,
                            ..
                        } => {
                            omega_isa_aarch64::runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
                                index_region,
                            )
                        }
                        CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. } => {
                            omega_isa_aarch64::runtime_frame_base_double_indexed_integer_write_clobbers()
                        }
                        CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. } => {
                            omega_isa_aarch64::runtime_machine_indexed_integer_write_clobbers()
                        }
                        CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => {
                            omega_isa_aarch64::runtime_machine_double_indexed_integer_write_clobbers(
                                outer_index_region,
                                inner_index_region,
                            )
                        }
                        CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed {
                            outer_index_region,
                            inner_index_region,
                            ..
                        } => omega_isa_aarch64::runtime_pointee_double_indexed_integer_write_clobbers(
                            outer_index_region,
                            inner_index_region,
                        ),
                        CompilerBodyPlaceIntegerWriteShape::General => return None,
                    },
                },
                MachineStateSet::empty(),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceAddressWrite {
            source,
            target_offset,
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
            compiler_place_address_write_register_writes(architecture, &source, target_offset)
                .ok()?,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::place_address_write_additional_machine_state()
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::runtime_place_address_write_additional_machine_state()
                }
            },
        ),
        CompilerInstructionValidationKind::CompilerBodyDataAddressWrite { .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
            match architecture {
                Architecture::X86_64 => omega_calling_conventions::RegisterSet::new([
                    omega_calling_conventions::MachineRegister::X86R14,
                    omega_calling_conventions::MachineRegister::X86R15,
                ]),
                Architecture::Aarch64 => omega_calling_conventions::RegisterSet::new([
                    omega_calling_conventions::MachineRegister::Aarch64X(16),
                    omega_calling_conventions::MachineRegister::Aarch64X(17),
                ]),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::CompilerBodyFunctionAddressStore { .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
            match architecture {
                Architecture::X86_64 => omega_calling_conventions::RegisterSet::new([
                    omega_calling_conventions::MachineRegister::X86R14,
                    omega_calling_conventions::MachineRegister::X86R15,
                ]),
                Architecture::Aarch64 => omega_calling_conventions::RegisterSet::new([
                    omega_calling_conventions::MachineRegister::Aarch64X(16),
                    omega_calling_conventions::MachineRegister::Aarch64X(17),
                ]),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::CompilerBodyConstantHostResult {
            result_offset,
            result_byte_size,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::constant_host_result_clobbers(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::CompilerBodyPlaceBinaryWrite {
            target,
            left,
            operator,
            right,
            ..
        } => {
            if architecture == Architecture::Aarch64
                && !matches!(
                    compiler_body_place_binary_write_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. },
                )
            {
                return None;
            }
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                    omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                    omega_isa_x86_64::place_binary_write_additional_machine_state(
                        runtime_value_operands,
                        left,
                        operator,
                        right,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
                    omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                    omega_isa_aarch64::place_binary_write_additional_machine_state(
                        runtime_value_operands,
                        left,
                        operator,
                        right,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyStorageConvertWrite { source, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                    omega_isa_x86_64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                    omega_isa_aarch64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceConvertWrite { source, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                    omega_isa_x86_64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
                    omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                    omega_isa_aarch64::storage_convert_write_additional_machine_state(
                        runtime_value_operands,
                        source,
                    ),
                ),
            }
        }
        _ => return None,
    };
    Some(parts)
}
