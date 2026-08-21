//! Derives fixed-mechanics, guard, entry, return, and dispatch footprints.

use super::*;

pub(super) fn control_entry_footprint_parts(
    architecture: Architecture,
    runtime_value_operands: &psi_arena::Arena<omega_target_operations::RuntimeValueOperand>,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Option<CompilerInstructionFootprintParts> {
    use omega_calling_conventions::MachineStateSet;
    use omega_machine_bytes::CompilerInstructionValidationKind;
    use omega_machine_instructions::BoundaryFootprintFragmentOrigin;

    let parts = match kind {
        CompilerInstructionValidationKind::InternalFunctionCall { .. } => match architecture {
            Architecture::X86_64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::internal_function_call_register_writes(),
                omega_isa_x86_64::internal_function_call_additional_machine_state(),
            ),
            Architecture::Aarch64 => (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_aarch64::internal_function_call_register_writes(),
                omega_isa_aarch64::internal_function_call_additional_machine_state(),
            ),
        },
        CompilerInstructionValidationKind::OutgoingStackAddressLoad { register, .. } => {
            if architecture != Architecture::X86_64 {
                return None;
            }
            (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::outgoing_stack_address_load_register_writes(register),
                omega_isa_x86_64::outgoing_stack_address_load_additional_machine_state(),
            )
        }
        CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy { .. } => {
            if architecture != Architecture::X86_64 {
                return None;
            }
            (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::entry_indirect_u64_to_outgoing_stack_copy_register_writes(),
                omega_isa_x86_64::entry_indirect_u64_to_outgoing_stack_copy_additional_machine_state(),
            )
        }
        CompilerInstructionValidationKind::OutgoingStackFrameReserve { .. }
        | CompilerInstructionValidationKind::OutgoingStackFrameRelease { .. } => {
            if architecture != Architecture::X86_64 {
                return None;
            }
            (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::outgoing_stack_frame_adjust_register_writes(),
                omega_isa_x86_64::outgoing_stack_frame_adjust_additional_machine_state(),
            )
        }
        CompilerInstructionValidationKind::OutgoingStackU64Write { .. } => {
            if architecture != Architecture::X86_64 {
                return None;
            }
            (
                BoundaryFootprintFragmentOrigin::CallReturnMechanics,
                omega_isa_x86_64::outgoing_stack_u64_write_register_writes(),
                omega_isa_x86_64::outgoing_stack_u64_write_additional_machine_state(),
            )
        }
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
        _ => return None,
    };
    Some(parts)
}
