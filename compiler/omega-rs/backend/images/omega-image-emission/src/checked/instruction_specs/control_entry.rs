//! Reconstructs fixed mechanics, guards, return transport, and entry transport.

use super::*;

pub(super) fn expected_control_entry_spec(
    architecture: Architecture,
    code: &omega_machine_bytes::EncodedMachineCode,
    function_instruction_count: usize,
    kind: omega_machine_bytes::CompilerInstructionValidationKind,
) -> Result<Option<CompilerInstructionSpec>, Diagnostic> {
    let spec = match kind {
                    omega_machine_bytes::CompilerInstructionValidationKind::InternalFunctionCall { target } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_internal_function_call_bytes().to_vec(),
                            Architecture::Aarch64 => omega_isa_aarch64::encode_internal_function_call_bytes().to_vec(),
                        },
                        77u8,
                        CompilerInstructionRelocationRecipe::InternalFunctionCall { target },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                        register,
                        stack_byte_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(
                                register,
                                stack_byte_offset,
                            )?.to_vec(),
                            Architecture::Aarch64 => return Err(Diagnostic::error(
                                "outgoing stack-address loads are supported only on x86-64",
                            )),
                        },
                        78u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                        source_register,
                        source_byte_offset,
                        stack_byte_offset,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                                source_register,
                                source_byte_offset,
                                stack_byte_offset,
                            )?.to_vec(),
                            Architecture::Aarch64 => return Err(Diagnostic::error(
                                "entry-indirect outgoing stack copies are supported only on x86-64",
                            )),
                        },
                        82u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackFrameReserve {
                        byte_count,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(byte_count)?,
                            Architecture::Aarch64 => return Err(Diagnostic::error(
                                "outgoing stack frames are supported only on x86-64",
                            )),
                        },
                        79u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_outgoing_stack_u64_write_bytes(
                                stack_byte_offset,
                                value,
                            )?.to_vec(),
                            Architecture::Aarch64 => return Err(Diagnostic::error(
                                "outgoing stack u64 writes are supported only on x86-64",
                            )),
                        },
                        81u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackFrameRelease {
                        byte_count,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(byte_count)?,
                            Architecture::Aarch64 => return Err(Diagnostic::error(
                                "outgoing stack frames are supported only on x86-64",
                            )),
                        },
                        80u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::FunctionEnter => (
                        Some(0),
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_function_enter_bytes().to_vec()
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_function_enter_bytes().to_vec()
                            }
                        },
                        1u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::FunctionReturn => (
                        Some(function_instruction_count - 1),
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_return_bytes().to_vec()
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_return_bytes().to_vec()
                            }
                        },
                        2u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchLoopEnter {
                        entry_dispatch_index,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_loop_enter_bytes(entry_dispatch_index)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_loop_enter_bytes(entry_dispatch_index)?.to_vec(),
                        },
                        3u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchCaseEnter {
                        dispatch_index,
                        skip_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_enter_bytes(dispatch_index, skip_byte_distance)?.to_vec(),
                        },
                        4u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchStaticGuard {
                        operator,
                        storage_region,
                        byte_offset,
                        byte_size,
                        expected_value,
                        skip_byte_distance,
                        is_float,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_guard_compare_static_bytes(
                                byte_offset,
                                byte_size,
                                expected_value,
                                skip_byte_distance,
                                operator,
                                is_float,
                            )?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_guard_compare_static_bytes(
                                byte_offset,
                                byte_size,
                                expected_value,
                                skip_byte_distance,
                                operator,
                                is_float,
                            )?,
                        },
                        8u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::PlacePairGuard {
                        left,
                        right,
                        byte_size,
                        failure_branch_distance,
                        operator,
                        is_float,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_compare(
                                &left,
                                &right,
                                byte_size,
                                failure_branch_distance,
                                operator,
                                is_float,
                            )?.0,
                            Architecture::Aarch64 => {
                                let left_offset = left.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-pair guard retained a non-direct place recipe",
                                ))?;
                                let right_offset = right.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-pair guard retained a non-direct place recipe",
                                ))?;
                                omega_isa_aarch64::encode_runtime_storage_compare_bytes(
                                    left_offset,
                                    right_offset,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                    is_float,
                                )?
                            }
                        },
                        9u8,
                        CompilerInstructionRelocationRecipe::PlacePair { left, right },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::PlaceValueGuard {
                        place,
                        byte_size,
                        expected_value,
                        failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_place_value_compare(
                                &place,
                                byte_size,
                                expected_value,
                                failure_branch_distance,
                                operator,
                            )?.0,
                            Architecture::Aarch64 => {
                                let byte_offset = place.const_offset().ok_or_else(|| Diagnostic::error(
                                    "final AArch64 place-value guard retained a non-direct place recipe",
                                ))?;
                                omega_isa_aarch64::encode_runtime_storage_value_compare_bytes(
                                    byte_offset,
                                    byte_size,
                                    expected_value,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                        },
                        10u8,
                        CompilerInstructionRelocationRecipe::PlaceValue(place),
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeTextLiteralGuard {
                        buffer_symbol,
                        literal,
                        failure_branch_distances,
                        delimiter_failure_branch_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_literal_compare(
                                    &literal,
                                    failure_branch_distances.into_iter(),
                                    delimiter_failure_branch_distance,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_literal_compare(
                                    &literal,
                                    failure_branch_distances.into_iter(),
                                    delimiter_failure_branch_distance,
                                )?
                            }
                        },
                        11u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextLiteral {
                            buffer_symbol,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeTextStorageGuard {
                        buffer_symbol,
                        source_region,
                        source_offset,
                        literal_len,
                        compare_failure_branch_distance,
                        delimiter_failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_text_storage_compare_bytes(
                                    source_offset,
                                    literal_len,
                                    compare_failure_branch_distance,
                                    operator
                                        == omega_target_operations::StateGuardOperator::NotEqual,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_text_storage_compare_bytes(
                                    source_offset,
                                    literal_len,
                                    compare_failure_branch_distance,
                                    delimiter_failure_branch_distance,
                                    operator
                                        == omega_target_operations::StateGuardOperator::NotEqual,
                                )?
                            }
                        },
                        12u8,
                        CompilerInstructionRelocationRecipe::RuntimeTextStorage {
                            buffer_symbol,
                            source_region,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeValueGuard {
                        left,
                        right,
                        byte_size,
                        failure_branch_distance,
                        operator,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_value_compare(
                                    &code.runtime_value_operands,
                                    left,
                                    right,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_value_compare(
                                    &code.runtime_value_operands,
                                    left,
                                    right,
                                    byte_size,
                                    failure_branch_distance,
                                    operator,
                                )?
                            }
                        },
                        13u8,
                        CompilerInstructionRelocationRecipe::RuntimeValue { left, right },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::ReturnRegisterIntegerWrite {
                        register,
                        byte_size,
                        value,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_return_register_integer_write_bytes(
                                    register,
                                    byte_size,
                                    value,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_return_register_integer_write_bytes(
                                    register,
                                    byte_size,
                                    value,
                                )?
                                .to_vec()
                            }
                        },
                        14u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::RuntimeStorageToReturnRegister {
                        register,
                        storage_region,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_runtime_storage_copy_to_return_register_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_runtime_storage_copy_to_return_register_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        15u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryArgumentRegisterWrite {
                        register,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_argument_register_write_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_argument_register_write_bytes(
                                    register,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        16u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryStackArgumentWrite {
                        stack_byte_offset,
                        byte_offset,
                        byte_size,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_stack_argument_write_bytes(
                                    stack_byte_offset,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_stack_argument_write_bytes(
                                    stack_byte_offset,
                                    byte_offset,
                                    byte_size,
                                )?
                            }
                        },
                        17u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryIndirectArgumentWrite {
                        pointer,
                        byte_offset,
                        byte_size,
                    } => {
                        let address_site = match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::entry_indirect_argument_frame_base_offset(pointer)
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::entry_indirect_argument_frame_base_offset(pointer)
                            }
                        };
                        (
                            None,
                            match architecture {
                                Architecture::X86_64 => {
                                    omega_isa_x86_64::encode_entry_indirect_argument_write_bytes(
                                        pointer,
                                        byte_offset,
                                        byte_size,
                                    )?
                                }
                                Architecture::Aarch64 => {
                                    omega_isa_aarch64::encode_entry_indirect_argument_write_bytes(
                                        pointer,
                                        byte_offset,
                                        byte_size,
                                    )?
                                }
                            },
                            18u8,
                            CompilerInstructionRelocationRecipe::StaticStorage {
                                storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                                address_site,
                            },
                        )
                    }
                    omega_machine_bytes::CompilerInstructionValidationKind::EntryArgumentsSliceDescriptorWrite {
                        descriptor_offset,
                        spill_offset,
                        byte_length,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => {
                                omega_isa_x86_64::encode_entry_arguments_slice_descriptor_write_bytes(
                                    descriptor_offset,
                                    spill_offset,
                                    byte_length,
                                )?
                            }
                            Architecture::Aarch64 => {
                                omega_isa_aarch64::encode_entry_arguments_slice_descriptor_write_bytes(
                                    descriptor_offset,
                                    spill_offset,
                                    byte_length,
                                )?
                            }
                        },
                        19u8,
                        CompilerInstructionRelocationRecipe::StaticStorage {
                            storage_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                            address_site: 0,
                        },
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchStateWrite {
                        dispatch_index,
                        case_leave_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_state_write_bytes(dispatch_index, case_leave_byte_distance)?.to_vec(),
                        },
                        5u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchCaseLeave {
                        loop_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(loop_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(loop_byte_distance)?.to_vec(),
                        },
                        7u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
                    omega_machine_bytes::CompilerInstructionValidationKind::DispatchForwardBranchSkip {
                        branch_arms_end_byte_distance,
                    } => (
                        None,
                        match architecture {
                            Architecture::X86_64 => omega_isa_x86_64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?,
                            Architecture::Aarch64 => omega_isa_aarch64::encode_dispatch_case_leave_bytes(branch_arms_end_byte_distance)?.to_vec(),
                        },
                        6u8,
                        CompilerInstructionRelocationRecipe::None,
                    ),
        _ => return Ok(None),
    };
    Ok(Some(spec))
}
