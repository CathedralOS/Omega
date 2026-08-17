//! Derives and composes exact compiler instruction footprints.

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

fn compiler_instruction_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<(
    omega_machine_instructions::BoundaryFootprintFragmentOrigin,
    omega_calling_conventions::StateFootprintEvidence,
)> {
    use omega_calling_conventions::{
        MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;
    use omega_target_operations::InstructionOperandLike;

    let (origin, registers, additional_state) = match kind {
        CompilerInstructionValidationKind::FunctionEnter => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::function_enter_register_writes(),
                omega_isa_x86_64::function_enter_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_aarch64::function_enter_register_writes(),
                omega_isa_aarch64::function_enter_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::FunctionReturn => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::return_register_writes(),
                omega_isa_x86_64::return_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_aarch64::return_register_writes(),
                omega_isa_aarch64::return_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::DispatchLoopEnter { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_loop_enter_register_writes(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::DispatchCaseEnter { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::DispatchScaffold,
                omega_isa_x86_64::dispatch_case_enter_register_writes(),
                omega_isa_x86_64::dispatch_case_enter_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::DispatchScaffold,
                omega_isa_aarch64::dispatch_case_enter_register_writes(),
                omega_isa_aarch64::dispatch_case_enter_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::DispatchStaticGuard { is_float, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                    omega_isa_x86_64::dispatch_guard_compare_static_register_writes(is_float),
                    omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::StaticGuardComparison,
                    omega_isa_aarch64::dispatch_guard_compare_static_register_writes(is_float),
                    omega_isa_aarch64::dispatch_guard_compare_static_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::PlacePairGuard {
            left,
            right,
            byte_size,
            is_float,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_x86_64::place_compare_register_writes(is_float),
                omega_isa_x86_64::place_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_aarch64::runtime_storage_compare_register_writes(
                    left.const_offset()?,
                    right.const_offset()?,
                    byte_size,
                    is_float,
                ),
                omega_isa_aarch64::runtime_storage_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::PlaceValueGuard { place, .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_x86_64::place_value_compare_register_writes(),
                omega_isa_x86_64::place_value_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 if place.const_offset().is_some() => (
                BoundaryFootprintFragmentOrigin::PlaceGuardComparison,
                omega_isa_aarch64::runtime_storage_value_compare_register_writes(),
                omega_isa_aarch64::runtime_storage_value_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => return None,
        },
        CompilerInstructionValidationKind::RuntimeTextLiteralGuard { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_x86_64::runtime_text_literal_compare_register_writes(),
                omega_isa_x86_64::runtime_text_literal_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_aarch64::runtime_text_literal_compare_register_writes(),
                omega_isa_aarch64::runtime_text_literal_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::RuntimeTextStorageGuard { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_x86_64::runtime_text_storage_compare_register_writes(),
                omega_isa_x86_64::runtime_text_storage_compare_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
                omega_isa_aarch64::runtime_text_storage_compare_register_writes(),
                omega_isa_aarch64::runtime_text_storage_compare_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::RuntimeValueGuard { left, right, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                    omega_isa_x86_64::runtime_value_compare_register_write_ceiling(),
                    omega_isa_x86_64::runtime_value_compare_additional_machine_state(
                        runtime_value_operands,
                        left,
                        right,
                    ),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
                    omega_isa_aarch64::runtime_value_compare_register_write_ceiling(),
                    omega_isa_aarch64::runtime_value_compare_additional_machine_state(
                        runtime_value_operands,
                        left,
                        right,
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::ReturnRegisterIntegerWrite { register, .. } => (
            BoundaryFootprintFragmentOrigin::ExitResultRegisters,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::return_register_integer_write_clobbers(register)
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::return_register_integer_write_clobbers(register)
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::RuntimeStorageToReturnRegister {
            register,
            byte_offset,
            byte_size,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::ExitResultRegisters,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::runtime_storage_copy_to_return_register_clobbers(register)
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::runtime_storage_copy_to_return_register_clobbers(
                        register,
                        byte_offset,
                        byte_size,
                    )
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryArgumentRegisterWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_argument_register_write_clobbers(),
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_argument_register_write_clobbers()
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryStackArgumentWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_stack_argument_write_clobbers(),
                Architecture::Aarch64 => omega_isa_aarch64::entry_stack_argument_write_clobbers(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryIndirectArgumentWrite { pointer, .. } => (
            BoundaryFootprintFragmentOrigin::EntryStorage,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::entry_indirect_argument_write_clobbers(),
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_indirect_argument_write_clobbers(pointer)
                }
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::EntryArgumentsSliceDescriptorWrite { .. } => (
            BoundaryFootprintFragmentOrigin::EntrySliceDescriptor,
            match architecture {
                Architecture::X86_64 => {
                    omega_isa_x86_64::entry_arguments_slice_descriptor_write_clobbers()
                }
                Architecture::Aarch64 => {
                    omega_isa_aarch64::entry_arguments_slice_descriptor_write_clobbers()
                }
            },
            MachineStateSet::empty(),
        ),
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
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundImmediateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDereferencedImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundIndirectCall {
            operands,
            mechanism,
            plan,
            ..
        } => {
            let dispatch_only = usize::from(matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::TableFunction { .. }
            ));
            let result_present = operands.len() == plan.parameters.len() + dispatch_only + 1;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => {
                    let mut registers = vec![MachineRegister::Aarch64X(16)];
                    if result_present
                        && let Some((_, result_offset, result_byte_size)) = operands
                            .first()
                            .and_then(InstructionOperandLike::runtime_scalar_integer)
                            .or_else(|| {
                                operands
                                    .first()
                                    .and_then(InstructionOperandLike::runtime_scalar_float)
                            })
                    {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                result_offset,
                                result_byte_size,
                            )
                            .as_slice(),
                        );
                    }
                    registers
                }
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundDataImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImport {
            plan, ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredFloatImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImport {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands
                    .first()?
                    .runtime_scalar_integer()
                    .or_else(|| operands.first()?.runtime_scalar_float())?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundAuthoredAggregateResult {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundOpenCreateImport {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain([MachineRegister::Aarch64X(16)])
                        .chain(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                result_offset,
                                result_byte_size,
                            )
                            .as_slice()
                            .iter()
                            .copied(),
                        ),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteRead {
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([MachineRegister::Aarch64X(20), MachineRegister::Aarch64X(9)]);
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeByteWrite {
            source_offset,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.push(MachineRegister::X86R14);
                    if matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    ) {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.push(MachineRegister::Aarch64X(20));
                    if source_offset > 4095 {
                        registers.push(MachineRegister::Aarch64X(9));
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyRuntimeLineRead {
            target_offset,
            target,
            mechanism,
            plan,
            get_std_handle,
            ..
        } => {
            use omega_target_operations::RuntimeTextReadTarget;

            let mut registers = plan.ordinary_clobbers.as_slice().to_vec();
            match architecture {
                Architecture::X86_64 => {
                    registers.extend([MachineRegister::X86R14, MachineRegister::X86R15]);
                    let is_import = matches!(
                        mechanism,
                        omega_calling_conventions::HostBindingMechanism::Import { .. }
                    );
                    if is_import || target == RuntimeTextReadTarget::StringDescriptor {
                        registers.push(MachineRegister::X86R13);
                    }
                    if is_import {
                        registers.push(MachineRegister::X86Rsp);
                        if let Some(handle) = get_std_handle {
                            registers.extend_from_slice(handle.plan.ordinary_clobbers.as_slice());
                        }
                    }
                }
                Architecture::Aarch64 => {
                    registers.extend([
                        MachineRegister::Aarch64X(20),
                        MachineRegister::Aarch64X(21),
                        MachineRegister::Aarch64X(22),
                        MachineRegister::Aarch64X(24),
                    ]);
                    match target {
                        RuntimeTextReadTarget::StringDescriptor => {
                            registers.push(MachineRegister::Aarch64X(16));
                            let direct_descriptor_stores = (target_offset + 8).is_multiple_of(8)
                                && (target_offset + 8) / 8 <= 4095;
                            if !direct_descriptor_stores && target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(9));
                            }
                        }
                        RuntimeTextReadTarget::BoundedByteBuffer => {
                            if target_offset + 8 > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                        RuntimeTextReadTarget::FixedByteArray => {
                            if target_offset > 4095 {
                                registers.push(MachineRegister::Aarch64X(19));
                            }
                        }
                    }
                }
            }
            let mut states = vec![
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ];
            if matches!(
                mechanism,
                omega_calling_conventions::HostBindingMechanism::Import { .. }
            ) {
                states.push(MachineState::StackPointer);
            }
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
                RegisterSet::new(registers),
                MachineStateSet::new(states),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImport { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
            RegisterSet::new(plan.ordinary_clobbers.as_slice().iter().copied().chain(
                match architecture {
                    Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                    Architecture::Aarch64 => vec![MachineRegister::Aarch64X(16)],
                },
            )),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundStorageImportResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) =
                operands.first()?.runtime_scalar_integer()?;
            let envelope_and_store_scratch = match architecture {
                Architecture::X86_64 => vec![MachineRegister::X86Rsp],
                Architecture::Aarch64 => Vec::from_iter(
                    [MachineRegister::Aarch64X(16)].into_iter().chain(
                        omega_isa_aarch64::constant_host_result_clobbers(
                            result_offset,
                            result_byte_size,
                        )
                        .as_slice()
                        .iter()
                        .copied(),
                    ),
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(envelope_and_store_scratch),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscall { plan, .. } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallDataArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallStorageArguments {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
            plan.ordinary_clobbers,
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultStorageArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallResultDataArguments {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let result_store = match architecture {
                Architecture::X86_64 => RegisterSet::default(),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(result_store.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::ControlState,
                ]),
            )
        }
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecArgument {
            plan,
            ..
        } => (
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
            RegisterSet::new(
                plan.ordinary_clobbers.as_slice().iter().copied().chain(
                    (architecture == Architecture::X86_64)
                        .then_some([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
                        .into_iter()
                        .flatten(),
                ),
            ),
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]),
        ),
        CompilerInstructionValidationKind::CompilerBodyOutboundSyscallTimespecResult {
            operands,
            plan,
            ..
        } => {
            let (_, result_offset, result_byte_size) = operands
                .first()
                .and_then(InstructionOperandLike::runtime_scalar_integer)?;
            let adapter_scratch = match architecture {
                Architecture::X86_64 => RegisterSet::new([MachineRegister::X86Rsp]),
                Architecture::Aarch64 => omega_isa_aarch64::constant_host_result_clobbers(
                    result_offset,
                    result_byte_size,
                ),
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
                RegisterSet::new(
                    plan.ordinary_clobbers
                        .as_slice()
                        .iter()
                        .copied()
                        .chain(adapter_scratch.as_slice().iter().copied()),
                ),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                    MachineState::ControlState,
                ]),
            )
        }
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
        CompilerInstructionValidationKind::CompilerBodyStorageBitFieldWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_x86_64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_x86_64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
                    omega_isa_aarch64::runtime_storage_bit_field_write_register_write_ceiling(),
                    omega_isa_aarch64::runtime_storage_bit_field_write_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferWrite {
            target, ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_write_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_write_additional_machine_state(&target),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_write_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_write_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_write_additional_machine_state(),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferLiteralAppend {
            target,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_literal_append_register_writes(&target),
                omega_isa_x86_64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_literal_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_literal_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_literal_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceBoundedBufferSourceAppend {
            target,
            source,
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                omega_isa_x86_64::place_bounded_buffer_source_append_register_writes(
                    &target, &source,
                ),
                omega_isa_x86_64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            Architecture::Aarch64 => {
                if !matches!(
                    compiler_body_place_bounded_buffer_source_append_shape(&target).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                        | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                ) || !matches!(
                    compiler_body_place_integer_write_shape(&source).ok()?,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                        | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                ) {
                    return None;
                }
                (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
                    omega_isa_aarch64::place_bounded_buffer_source_append_register_write_ceiling(),
                    omega_isa_aarch64::place_bounded_buffer_source_append_additional_machine_state(
                    ),
                )
            }
        },
        CompilerInstructionValidationKind::CompilerBodyPlaceStringWrite { target, .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                    omega_isa_x86_64::place_string_write_register_writes(&target),
                    omega_isa_x86_64::place_string_write_additional_machine_state(&target),
                ),
                Architecture::Aarch64 => {
                    if !matches!(
                        compiler_body_place_string_write_shape(&target).ok()?,
                        CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                            | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineIndexed { .. }
                            | CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed { .. }
                    ) {
                        return None;
                    }
                    (
                        BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
                        omega_isa_aarch64::place_string_write_register_write_ceiling(),
                        omega_isa_aarch64::place_string_write_additional_machine_state(),
                    )
                }
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireLiteralByteAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                    omega_isa_x86_64::append_wire_literal_byte_clobbers(),
                    omega_isa_x86_64::append_wire_literal_byte_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
                    omega_isa_aarch64::append_wire_literal_byte_clobbers(),
                    MachineStateSet::empty(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireScalarVarintAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                    omega_isa_x86_64::append_wire_scalar_varint_clobbers(),
                    omega_isa_x86_64::append_wire_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
                    omega_isa_aarch64::append_wire_scalar_varint_clobbers(),
                    MachineStateSet::empty(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireTextBytesAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                    omega_isa_x86_64::append_wire_text_bytes_clobbers(),
                    omega_isa_x86_64::append_wire_text_bytes_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
                    omega_isa_aarch64::append_wire_text_bytes_clobbers(),
                    omega_isa_aarch64::append_wire_text_bytes_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireScalarSliceAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                    omega_isa_x86_64::append_wire_scalar_slice_clobbers(),
                    omega_isa_x86_64::append_wire_scalar_slice_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
                    omega_isa_aarch64::append_wire_scalar_slice_clobbers(),
                    omega_isa_aarch64::append_wire_scalar_slice_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintAppend {
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                omega_isa_x86_64::append_wire_repeated_scalar_varint_clobbers(),
                omega_isa_x86_64::append_wire_repeated_scalar_varint_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
                omega_isa_aarch64::append_wire_repeated_scalar_varint_clobbers(),
                omega_isa_aarch64::append_wire_repeated_scalar_varint_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireExpectedByteRead {
            read_offset,
            ok_offset,
            ..
        } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                omega_isa_x86_64::read_wire_expected_byte_clobbers(),
                omega_isa_x86_64::read_wire_expected_byte_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
                omega_isa_aarch64::read_wire_expected_byte_clobbers(read_offset, ok_offset),
                omega_isa_aarch64::read_wire_expected_byte_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireScalarVarintRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                    omega_isa_x86_64::read_wire_scalar_varint_clobbers(),
                    omega_isa_x86_64::read_wire_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
                    omega_isa_aarch64::read_wire_scalar_varint_clobbers(),
                    omega_isa_aarch64::read_wire_scalar_varint_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireByteSliceRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                    omega_isa_x86_64::read_wire_byte_slice_clobbers(),
                    omega_isa_x86_64::read_wire_byte_slice_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
                    omega_isa_aarch64::read_wire_byte_slice_clobbers(),
                    omega_isa_aarch64::read_wire_byte_slice_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireNestedOpen { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                    omega_isa_x86_64::read_wire_nested_open_clobbers(),
                    omega_isa_x86_64::read_wire_nested_open_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
                    omega_isa_aarch64::read_wire_nested_open_clobbers(),
                    omega_isa_aarch64::read_wire_nested_open_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyWireNestedClose { .. } => match architecture
        {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                omega_isa_x86_64::read_wire_nested_close_clobbers(),
                omega_isa_x86_64::read_wire_nested_close_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
                omega_isa_aarch64::read_wire_nested_close_clobbers(),
                omega_isa_aarch64::read_wire_nested_close_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::CompilerBodyWireRepeatedScalarVarintRead { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_clobbers(),
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_clobbers(),
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_additional_machine_state(),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextBufferMaterialize { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_buffer_materialize_register_writes(),
                    omega_isa_x86_64::place_text_buffer_materialize_additional_machine_state(
                        &target,
                    ),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_literal_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_literal_append_register_writes(&target),
                    omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_literal_append_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredAppend { target, .. } => {
            let shape = compiler_body_place_integer_write_shape(&target).ok()?;
            let (registers, additional_state) = match (architecture, shape) {
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::X86_64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. },
                ) => (
                    omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (Architecture::X86_64, _) => (
                    omega_isa_x86_64::place_text_stored_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::Direct { .. }
                    | CompilerBodyPlaceIntegerWriteShape::Pointee { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameIndexed { .. }
                    | CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                (
                    Architecture::Aarch64,
                    CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed { .. },
                ) => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                _ => return None,
            };
            (
                BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                registers,
                additional_state,
            )
        }
        CompilerInstructionValidationKind::CompilerBodyTextLiteralSegmentWrite { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_segment_write_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_segment_write_additional_machine_state(
                    ),
                ),
            }
        }
        CompilerInstructionValidationKind::CompilerBodyTextStoredSuffixAppend { .. } => {
            match architecture {
                Architecture::X86_64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_x86_64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
                Architecture::Aarch64 => (
                    BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
                    omega_isa_aarch64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_suffix_append_additional_machine_state(),
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
        CompilerInstructionValidationKind::DispatchStateWrite { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_state_write_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_state_write_register_writes(),
            },
            MachineStateSet::empty(),
        ),
        CompilerInstructionValidationKind::DispatchForwardBranchSkip { .. }
        | CompilerInstructionValidationKind::DispatchCaseLeave { .. } => (
            BoundaryFootprintFragmentOrigin::DispatchScaffold,
            match architecture {
                Architecture::X86_64 => omega_isa_x86_64::dispatch_case_leave_register_writes(),
                Architecture::Aarch64 => omega_isa_aarch64::dispatch_case_leave_register_writes(),
            },
            MachineStateSet::empty(),
        ),
    };
    Some((
        origin,
        StateFootprintEvidence::new(registers, additional_state),
    ))
}

pub(super) fn require_compiler_instruction_footprint(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
    selected_instruction_index: u32,
) -> Result<
    (
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    ),
    Diagnostic,
> {
    compiler_instruction_footprint(architecture, runtime_value_operands, kind).ok_or_else(|| {
        Diagnostic::error(format!(
            "compiler instruction #{selected_instruction_index} has a retained final-byte validation identity but no target footprint derivation"
        ))
    })
}

pub(super) fn validate_compiler_composed_footprint(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<u64, Diagnostic> {
    let final_footprint = omega_calling_conventions::compose_state_footprints(
        derived.iter().map(|(_, evidence)| evidence),
    );
    let retained_footprint = omega_calling_conventions::compose_state_footprints(
        semantics
            .boundaries
            .footprints
            .fragments
            .iter()
            .filter(|fragment| {
                fragment.origin
                    != omega_machine_instructions::BoundaryFootprintFragmentOrigin::CheckedAssemblyCatalog
            })
            .map(|fragment| &fragment.evidence),
    );
    if final_footprint != retained_footprint {
        return Err(Diagnostic::error(format!(
            "complete final compiler-row footprint does not equal the StatePlan-validated semantic union: retained={retained_footprint:?}, replayed={final_footprint:?}"
        )));
    }
    Ok(final_footprint.evidence_fingerprint())
}

pub(super) fn validate_compiler_body_specification_footprints(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let has_body_rows = derived.iter().any(|(origin, _)| {
        matches!(
            origin,
            BoundaryFootprintFragmentOrigin::DispatchScaffold
                | BoundaryFootprintFragmentOrigin::StaticGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison
                | BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison
                | BoundaryFootprintFragmentOrigin::PlaceGuardComparison
                | BoundaryFootprintFragmentOrigin::ExitResultRegisters
                | BoundaryFootprintFragmentOrigin::EntryStorage
                | BoundaryFootprintFragmentOrigin::EntrySliceDescriptor
                | BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation
                | BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument
                | BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose
                | BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead
                | BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite
                | BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite
        )
    });
    let boundary_contract_fingerprint = if !has_body_rows {
        0
    } else {
        semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint
            .ok_or_else(|| {
                Diagnostic::error(
                    "final body-specification footprint rows have no StatePlan boundary-contract identity",
                )
            })?
    };
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    for (tag, origin) in [
        (1u8, BoundaryFootprintFragmentOrigin::DispatchScaffold),
        (2u8, BoundaryFootprintFragmentOrigin::StaticGuardComparison),
        (
            3u8,
            BoundaryFootprintFragmentOrigin::RuntimeTextGuardComparison,
        ),
        (
            4u8,
            BoundaryFootprintFragmentOrigin::RuntimeValueGuardComparison,
        ),
        (5u8, BoundaryFootprintFragmentOrigin::PlaceGuardComparison),
        (6u8, BoundaryFootprintFragmentOrigin::ExitResultRegisters),
        (7u8, BoundaryFootprintFragmentOrigin::EntryStorage),
        (8u8, BoundaryFootprintFragmentOrigin::EntrySliceDescriptor),
        (9u8, BoundaryFootprintFragmentOrigin::ExitIndirectResultCopy),
        (10u8, BoundaryFootprintFragmentOrigin::CompilerBodyPlaceCopy),
        (
            11u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceIntegerWrite,
        ),
        (
            12u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBinaryWrite,
        ),
        (
            13u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageConvertWrite,
        ),
        (
            14u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyStorageBitFieldWrite,
        ),
        (
            15u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceBoundedBufferWrite,
        ),
        (
            16u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceStringWrite,
        ),
        (
            17u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyTextAssemblyWrite,
        ),
        (
            18u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyPlaceAddressWrite,
        ),
        (
            19u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyConstantHostResult,
        ),
        (
            20u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImport,
        ),
        (
            21u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundImmediateImportResult,
        ),
        (
            22u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundFloatImportResult,
        ),
        (
            23u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImport,
        ),
        (
            24u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundStorageImportResult,
        ),
        (
            25u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscall,
        ),
        (
            26u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResult,
        ),
        (
            27u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallStorageArguments,
        ),
        (
            28u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultStorageArguments,
        ),
        (
            29u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallDataArguments,
        ),
        (
            30u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallResultDataArguments,
        ),
        (
            31u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecArgument,
        ),
        (
            32u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundSyscallTimespecResult,
        ),
        (
            33u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDereferencedImportResult,
        ),
        (
            34u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImport,
        ),
        (
            35u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundDataImportResult,
        ),
        (
            36u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImport,
        ),
        (
            37u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredImportResult,
        ),
        (
            38u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImport,
        ),
        (
            39u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredFloatImportResult,
        ),
        (
            40u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImport,
        ),
        (
            41u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateImportResult,
        ),
        (
            42u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundAuthoredAggregateResult,
        ),
        (
            43u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundOpenCreateImport,
        ),
        (
            44u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteRead,
        ),
        (
            45u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeByteWrite,
        ),
        (
            46u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyRuntimeLineRead,
        ),
        (
            47u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireLiteralByteAppend,
        ),
        (
            48u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintAppend,
        ),
        (
            49u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireExpectedByteRead,
        ),
        (
            50u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarVarintRead,
        ),
        (
            51u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireTextBytesAppend,
        ),
        (
            52u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireScalarSliceAppend,
        ),
        (
            53u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintAppend,
        ),
        (
            54u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireByteSliceRead,
        ),
        (
            55u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedOpen,
        ),
        (
            56u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireNestedClose,
        ),
        (
            57u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyWireRepeatedScalarVarintRead,
        ),
        (
            58u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyAtomicOperation,
        ),
        (
            59u8,
            BoundaryFootprintFragmentOrigin::CompilerBodyOutboundIndirectCall,
        ),
    ] {
        let evidence_rows = derived
            .iter()
            .filter_map(|(row_origin, evidence)| (*row_origin == origin).then_some(evidence))
            .collect::<Vec<_>>();
        let retained = semantics
            .boundaries
            .footprints
            .fragments
            .iter()
            .filter(|fragment| fragment.origin == origin)
            .collect::<Vec<_>>();
        if evidence_rows.is_empty() {
            let retains_valid_empty_entry_storage = origin
                == BoundaryFootprintFragmentOrigin::EntryStorage
                && retained.len() == 1
                && retained[0].evidence.registers().as_slice().is_empty()
                && retained[0].evidence.machine_state().is_empty();
            if !retained.is_empty() && !retains_valid_empty_entry_storage {
                return Err(Diagnostic::error(format!(
                    "retained {origin:?} footprint has no final target-specification instruction rows"
                )));
            }
            continue;
        }
        let composed = compose_state_footprints(evidence_rows.iter().copied());
        if retained.len() != 1 || retained[0].evidence != composed {
            return Err(Diagnostic::error(format!(
                "final {origin:?} target-specification footprint does not match its StatePlan-validated semantic fragment: retained={:?}, replayed={composed:?}",
                retained
                    .iter()
                    .map(|fragment| &fragment.evidence)
                    .collect::<Vec<_>>()
            )));
        }
        fingerprint_into(&mut fingerprint, &[tag]);
        fingerprint_into(
            &mut fingerprint,
            &(evidence_rows.len() as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &composed.evidence_fingerprint().to_le_bytes(),
        );
    }
    Ok((boundary_contract_fingerprint, fingerprint))
}

pub(super) fn validate_compiler_fixed_mechanics_footprint(
    semantics: &omega_machine_bytes::EncodedMachineSemanticSummary,
    derived: &[(
        omega_machine_instructions::BoundaryFootprintFragmentOrigin,
        omega_calling_conventions::StateFootprintEvidence,
    )],
) -> Result<(u64, u64), Diagnostic> {
    use omega_calling_conventions::compose_state_footprints;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let evidence_rows = derived
        .iter()
        .filter_map(|(origin, evidence)| {
            (*origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics).then_some(evidence)
        })
        .collect::<Vec<_>>();
    if evidence_rows.is_empty() {
        return Ok((0, 0xcbf2_9ce4_8422_2325u64));
    }
    let boundary_contract_fingerprint = semantics
        .boundaries
        .footprints
        .boundary_contract_fingerprint
        .ok_or_else(|| {
            Diagnostic::error(
                "final call-return footprint rows have no StatePlan boundary-contract identity",
            )
        })?;
    let retained = semantics
        .boundaries
        .footprints
        .fragments
        .iter()
        .filter(|fragment| fragment.origin == BoundaryFootprintFragmentOrigin::CallReturnMechanics)
        .collect::<Vec<_>>();
    let composed = compose_state_footprints(evidence_rows.iter().copied());
    if retained.len() != 1 || retained[0].evidence != composed {
        return Err(Diagnostic::error(
            "final CallReturnMechanics target-specification footprint does not match its StatePlan-validated semantic fragment",
        ));
    }
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut fingerprint,
        &boundary_contract_fingerprint.to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &(evidence_rows.len() as u64).to_le_bytes(),
    );
    fingerprint_into(
        &mut fingerprint,
        &composed.evidence_fingerprint().to_le_bytes(),
    );
    Ok((boundary_contract_fingerprint, fingerprint))
}
